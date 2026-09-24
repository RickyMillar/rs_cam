//! Byte-parity rig for the adaptive3d emission — the guard the design-debt
//! rows CUT-04 and CUT-09 are proven by.
//!
//! ## Why this file exists
//!
//! CUT-04 regroups `Adaptive3dParams`'s thirty flat fields into three named
//! groups, and CUT-09 cuts `clear_z_level_agent_2d_slice` into its three
//! named stages. Both are mechanical re-shapes with the same failure mode: a
//! field threaded into the wrong slot, or a stage that reads an accumulator
//! one statement too early, changes the emitted motion in silence. The
//! existing adaptive3d sentries measure aggregates — a coverage fraction, a
//! plunge count, a parity percentage — so a small coordinate shift passes
//! them.
//!
//! This rig measures the emission itself. It serialises every move of a
//! generated toolpath as the raw `f64` bit pattern of its coordinates, its
//! move type and its intent tag, then holds the whole dump against a
//! committed fixture. Nothing about the toolpath can move without a line of
//! the dump moving with it.
//!
//! ## The four cases
//!
//! One per `ClearingStrategy3d` variant, so the AgentSearch arm CUT-09 cuts
//! is measured, and so is the `ContourSpiral` arm that dispatches through it.
//! The cases deliberately turn the optional dials ON, because a rig that
//! runs only the all-`None` parameter set cannot see a mis-threaded dial:
//!
//! | case | strategy | dials exercised |
//! |---|---|---|
//! | `contour_parallel` | `ContourParallel` | the plain baseline |
//! | `adaptive` | `Adaptive` | `detect_flat_areas`, `z_blend` |
//! | `agent_search` | `AgentSearch` | `boundary`, stay-down, `min_region_cut_length_mm` |
//! | `contour_spiral` | `ContourSpiral` | `trochoid_cap_mult`, `RegionOrdering::ByArea` |
//!
//! ## Determinism
//!
//! `emission_is_deterministic_within_one_process` generates the AgentSearch
//! case twice and compares the two dumps. A generator that iterated a
//! `HashMap` into emission order would fail there, and the fixture would be
//! worthless. Run the file twice to cover the cross-process case: the
//! fixture comparison is itself that check.
//!
//! ## Re-blessing
//!
//! Set `ADAPTIVE3D_BYTE_PARITY_BLESS=1` to rewrite the fixture. Do that only
//! when a change is MEANT to move the motion, and say so in the commit body.
//! A refactor row re-blesses nothing.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, RegionOrdering, adaptive_3d_toolpath_with_cancel,
};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveType, Toolpath};

const TOOL_RADIUS: f64 = 2.0;
const STOCK_TOP_Z: f64 = 12.0;
const SAFE_Z: f64 = 16.0;
const MESH_RADIUS: f64 = 10.0;
const MESH_DIVISIONS: usize = 10;

fn hemisphere() -> (TriangleMesh, SpatialIndex) {
    let mesh = make_test_hemisphere(MESH_RADIUS, MESH_DIVISIONS);
    let index = SpatialIndex::build(&mesh, 5.0);
    (mesh, index)
}

/// The plain parameter set. Every optional dial is off; the four cases below
/// switch the ones they measure back on.
fn base_params(strategy: ClearingStrategy3d) -> Adaptive3dParams {
    Adaptive3dParams {
        geometry: Adaptive3dGeometry {
            tool_radius: TOOL_RADIUS,
            envelope_radius: TOOL_RADIUS,
            stepover: 1.5,
            tolerance: 0.5,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: Some((-13.0, -13.0, 13.0, 13.0)),
        },
        depth: Adaptive3dDepth {
            depth_per_pass: 3.0,
            stock_to_leave: 0.5,
            stock_top_z: STOCK_TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: Some(0.0),
            stay_down_clearance_mm: 0.5,
        },
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: SAFE_Z,
        entry_style: EntryStyle3d::Plunge,
        initial_stock: None,
        clearing_strategy: strategy,
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        z_blend: false,
    }
}

fn case_params(case: &str) -> Adaptive3dParams {
    match case {
        "contour_parallel" => base_params(ClearingStrategy3d::ContourParallel),
        "adaptive" => {
            let mut p = base_params(ClearingStrategy3d::Adaptive);
            p.depth.detect_flat_areas = true;
            p.z_blend = true;
            p
        }
        "agent_search" => {
            let mut p = base_params(ClearingStrategy3d::AgentSearch);
            p.geometry.boundary = Some(Polygon2::rectangle(-12.0, -12.0, 6.0, 12.0));
            p.linking.max_stay_down_distance_mm = Some(8.0);
            p.linking.min_region_cut_length_mm = 3.0;
            p
        }
        "contour_spiral" => {
            let mut p = base_params(ClearingStrategy3d::ContourSpiral);
            p.linking.region_ordering = RegionOrdering::ByArea;
            p.trochoid_cap_mult = 2.0;
            p
        }
        other => panic!("unknown case {other}"),
    }
}

const CASES: [&str; 4] = [
    "contour_parallel",
    "adaptive",
    "agent_search",
    "contour_spiral",
];

fn generate(case: &str) -> Toolpath {
    let (mesh, index) = hemisphere();
    let cutter = FlatEndmill::new(TOOL_RADIUS * 2.0, 30.0);
    let params = case_params(case);
    adaptive_3d_toolpath_with_cancel(&mesh, &index, &cutter, &params, &(|| false))
        .expect("generation must not be cancelled")
}

/// One move, one line, every number as its raw bit pattern. A hash would be
/// shorter; a hash cannot say WHICH move moved.
fn dump(case: &str, tp: &Toolpath) -> String {
    let mut out = format!("# case {case}\n# moves {}\n", tp.moves.len());
    for (i, m) in tp.moves.iter().enumerate() {
        let kind = match m.move_type {
            MoveType::Rapid => "rapid".to_owned(),
            MoveType::Linear { feed_rate } => format!("linear f={:016x}", feed_rate.to_bits()),
            MoveType::ArcCW { i, j, feed_rate } => format!(
                "arccw i={:016x} j={:016x} f={:016x}",
                i.to_bits(),
                j.to_bits(),
                feed_rate.to_bits()
            ),
            MoveType::ArcCCW { i, j, feed_rate } => format!(
                "arcccw i={:016x} j={:016x} f={:016x}",
                i.to_bits(),
                j.to_bits(),
                feed_rate.to_bits()
            ),
        };
        out.push_str(&format!(
            "{i} {:016x} {:016x} {:016x} {kind} {:?}\n",
            m.target.x.to_bits(),
            m.target.y.to_bits(),
            m.target.z.to_bits(),
            m.intent
        ));
    }
    out
}

fn fixture_path(case: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/adaptive3d_emission")
        .join(format!("{case}.txt"))
}

fn first_difference(expected: &str, actual: &str) -> String {
    for (n, (e, a)) in expected.lines().zip(actual.lines()).enumerate() {
        if e != a {
            return format!("line {}:\n  expected: {e}\n  actual:   {a}", n + 1);
        }
    }
    format!(
        "line counts differ: expected {} lines, actual {} lines",
        expected.lines().count(),
        actual.lines().count()
    )
}

fn check(case: &str) {
    let tp = generate(case);
    assert!(
        !tp.moves.is_empty(),
        "case {case} emitted nothing; the fixture would guard an empty toolpath"
    );
    let actual = dump(case, &tp);
    let path = fixture_path(case);
    if std::env::var("ADAPTIVE3D_BYTE_PARITY_BLESS").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing fixture {} ({e}); re-bless with ADAPTIVE3D_BYTE_PARITY_BLESS=1",
            path.display()
        )
    });
    assert!(
        expected == actual,
        "case {case}: the emitted motion moved.\n{}",
        first_difference(&expected, &actual)
    );
}

#[test]
fn contour_parallel_emission_is_byte_identical() {
    check("contour_parallel");
}

#[test]
fn adaptive_emission_is_byte_identical() {
    check("adaptive");
}

#[test]
fn agent_search_emission_is_byte_identical() {
    check("agent_search");
}

#[test]
fn contour_spiral_emission_is_byte_identical() {
    check("contour_spiral");
}

/// The fixture is only worth something if the generator is deterministic.
#[test]
fn emission_is_deterministic_within_one_process() {
    for case in CASES {
        let first = dump(case, &generate(case));
        let second = dump(case, &generate(case));
        assert!(
            first == second,
            "case {case} is not deterministic in one process; the fixture cannot pin it.\n{}",
            first_difference(&first, &second)
        );
    }
}
