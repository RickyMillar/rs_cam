//! Integration step 7.4, lead condition 4 of option B: every cut of a
//! Geodesic Adaptive plan that carries a feed cap keeps it through every
//! dressup.
//!
//! The arm writes each turn's load cap on its own cut (`Move::feed_cap`,
//! design part F). The census plans the island pocket of the research C12
//! judge, with the planner's segment merge on, and runs the plan through
//! the whole production pipeline (`apply_dressups`: every step of
//! `DRESSUP_PIPELINE` that a 3D Rough allows, each switched on) and then
//! through the feed modulator, which the session runs after the pipeline.
//!
//! The census reads the output against the plan:
//! 1. a fed line whose two ends are the two ends of a planned cut carries
//!    the cap of that cut;
//! 2. a fed arc carries a cap that a planned cut at each of its two ends
//!    carries (arc fit joins only cuts with equal caps);
//! 3. no fed move leaves the pipeline, or the modulator after it, with a
//!    feed over its cap.
//!
//! The per-stage proof that a cap binds is
//! `a_feed_cap_binds_every_feed_stage_c38`; this file holds the census on
//! the arm's own plan.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::collections::{HashMap, HashSet};

use common::meshes::height_field;
use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, KeepDownRoute, RegionOrdering, StepSplit,
    adaptive_3d_toolpath_output_with_cancel,
};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{
    ArcFitParams, DressupConfig, LinkDressupParams, SegmentMergeParams,
};
use rs_cam_core::compute::execute::{DressupContext, apply_dressups};
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::dressup::feed_modulation::{
    ChiploadBand, ModulationContext, ModulationStrategy, PerMoveEngagement, adaptive_feed_modulate,
};
use rs_cam_core::geo::P3;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::trace::transform_provenance::ReconcileSet;

// The research C12 judge's fixture (`geodesic_adaptive_keeps_the_research_
// geometry_s7_4.rs`, island pocket, one level).
const TOOL_RADIUS: f64 = 3.0;
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const POCKET_HALF_X: f64 = 34.0;
const POCKET_HALF_Y: f64 = 24.0;
const ISLAND_RADIUS: f64 = 5.0;
const FEED: f64 = 1000.0;
const PLUNGE: f64 = 500.0;
const SAFE_Z: f64 = 15.0;
/// Two points are one vertex when they agree to this (mm): the dressups
/// copy a target, they do not compute it again.
const SAME_POINT: f64 = 1e-6;

fn pocket() -> TriangleMesh {
    height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.hypot(y) <= ISLAND_RADIUS {
            TOP_Z
        } else if x.abs() <= POCKET_HALF_X && y.abs() <= POCKET_HALF_Y {
            FLOOR_Z
        } else {
            TOP_Z
        }
    })
}

fn params() -> Adaptive3dParams {
    Adaptive3dParams {
        entry_clearance_mm: 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: TOOL_RADIUS,
            envelope_radius: TOOL_RADIUS,
            stepover: 2.0,
            tolerance: 0.1,
            segment_merge_tolerance: Some(SegmentMergeParams::default().tolerance),
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
            centre_boundary: Vec::new(),
            gouge_probe: None,
        },
        depth: Adaptive3dDepth {
            depth_per_pass: TOP_Z - FLOOR_Z,
            stock_to_leave: 0.5,
            stock_top_z: TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
            rest_top_z: None,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: None,
            stay_down_clearance_mm: 0.5,
            step_split: Some(StepSplit::StockDepth),
            keep_down_route: Some(KeepDownRoute::CutFeed),
            level_contour: None,
            rest_region_filter: None,
        },
        feed_rate: FEED,
        plunge_rate: PLUNGE,
        safe_z: SAFE_Z,
        entry_style: EntryStyle3d::Plunge,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::GeodesicAdaptive,
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
    }
}

fn plan(mesh: &TriangleMesh, cutter: &FlatEndmill) -> Toolpath {
    let index = SpatialIndex::build(mesh, 10.0);
    adaptive_3d_toolpath_output_with_cancel(mesh, &index, cutter, &params(), &|| false, None)
        .expect("the plan is not cancelled")
        .toolpath
}

/// The board the plan cuts: the mesh footprint, from the floor of the mesh
/// to the stock top.
fn board() -> TriDexelStock {
    TriDexelStock::from_stock(
        -MESH_HALF, -MESH_HALF, MESH_HALF, MESH_HALF, 0.0, TOP_Z, MESH_STEP,
    )
}

/// Every dressup step a 3D Rough allows, each switched on.
fn every_dressup() -> DressupConfig {
    let mut cfg = DressupConfig::for_op(OperationType::Adaptive3d);
    cfg.link_moves = Some(LinkDressupParams::default());
    cfg.arc_fitting = Some(ArcFitParams::default());
    cfg.segment_merge = Some(SegmentMergeParams::default());
    cfg.feed_optimization = true;
    cfg.optimize_rapid_order = true;
    cfg
}

/// The production pipeline.
fn dress(planned: &Toolpath, cutter: &FlatEndmill) -> Toolpath {
    let cfg = every_dressup();
    let link = MachineProfile::default().link_kinematics();
    let prior = board();
    let mut feed_stock = board();
    apply_dressups(
        AnnotatedToolpath::new(planned.clone()),
        DressupContext {
            material: &rs_cam_core::material::Material::default(),
            cfg: &cfg,
            nominal_feed_rate: FEED,
            plunge_rate_mm_min: Some(PLUNGE),
            ramp_feed_rate_mm_min: None,
            tool_diameter: 2.0 * TOOL_RADIUS,
            safe_z: SAFE_Z,
            stock_top: TOP_Z,
            prior_stock: Some(&prior),
            link_kinematics: Some(&link),
            op_cutting_ceiling: Some(
                MachineProfile::default().op_cutting_ceiling(cfg.feed_max_rate),
            ),
            feed_opt_stock: Some(&mut feed_stock),
            cutter: Some(cutter),
            entry_surface: None,
            transform_capabilities: OperationType::Adaptive3d.capabilities().transform,
            entry: OperationType::Adaptive3d.registry_entry().entry,
            debug_ctx: None,
            semantic_ctx: None,
            entry_containment: None,
        },
        &mut ReconcileSet::empty(),
    )
    .toolpath
}

/// The feed modulator, which the session runs after the pipeline.
fn modulate(mut tp: Toolpath) -> Toolpath {
    // A full slot on every cut: the modulator wants the band limit on each.
    let engagement: Vec<PerMoveEngagement> = tp
        .moves
        .iter()
        .map(|m| {
            if m.move_type.is_cutting() {
                PerMoveEngagement {
                    radial_woc_fraction: 1.0,
                    axial_doc_fraction: 1.0,
                    axial_doc_mm: TOP_Z - FLOOR_Z,
                    thinning_lift: None,
                }
            } else {
                PerMoveEngagement::default()
            }
        })
        .collect();
    let kinematics = MachineKinematics::shapeoko_xxl_stock();
    let ctx = ModulationContext {
        spindle_rpm: 18_000.0,
        flute_count: 2,
        cutting_ceiling: MachineProfile::default().cutting_ceiling(),
        rapid_feed_mm_min: 10_000.0,
        chipload_band: Some(ChiploadBand::new(0.02, 0.08).expect("a valid band")),
        radial_thinning: rs_cam_core::dressup::feed_modulation::RadialThinning::Applied,
        kinematics: &kinematics,
        strategy: ModulationStrategy::ConstrainedMax,
        feed_scale: 1.0,
        deflection_inputs: None,
        power_inputs: None,
        force_budget_inputs: None,
        sample_engagements: &[],
        nominal_axial_doc_mm: TOP_Z - FLOOR_Z,
        plunge_rate_mm_min: PLUNGE,
    };
    adaptive_feed_modulate(&mut tp, &engagement, &ctx).expect("the modulator runs");
    tp
}

/// A point as a hash key.
type Vertex = [i64; 3];
/// The caps that the planned cuts carry, as hash keys.
type Caps = HashSet<Option<u64>>;

/// A point as a hash key, to [`SAME_POINT`].
fn key(p: P3) -> Vertex {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the fixture's coordinates are tens of mm, far inside i64 at 1e-6 mm"
    )]
    let k = |v: f64| (v / SAME_POINT).round() as i64;
    [k(p.x), k(p.y), k(p.z)]
}

/// The fed moves of `tp` as `(start, end, move)` triples.
fn fed(tp: &Toolpath) -> impl Iterator<Item = (P3, P3, &rs_cam_core::toolpath::Move)> {
    tp.moves
        .windows(2)
        .map(|w| (w[0].target, w[1].target, &w[1]))
        .filter(|(_, _, m)| m.move_type.is_cutting())
}

/// No fed move of `tp` leaves `stage` with a feed over its cap.
fn assert_no_feed_over_its_cap(tp: &Toolpath, stage: &str) {
    for (i, (_, _, m)) in fed(tp).enumerate() {
        if let Some(cap) = m.feed_cap {
            let feed = m.move_type.feed_rate().expect("a fed move has a feed");
            assert!(
                feed <= cap,
                "{stage}: fed move {i} has the feed {feed}, over its cap {cap}"
            );
        }
    }
}

/// The cap as a hash key: `None` stays `None`.
fn cap_key(cap: Option<f64>) -> Option<u64> {
    cap.map(f64::to_bits)
}

#[test]
fn a_geodesic_cap_survives_every_dressup() {
    let mesh = pocket();
    let cutter = FlatEndmill::new(2.0 * TOOL_RADIUS, 25.0);
    let planned = plan(&mesh, &cutter);

    // The planned cuts: each line by its two ends (either order), and the
    // caps of the cuts that meet at each vertex.
    let mut by_ends: HashMap<(Vertex, Vertex), Caps> = HashMap::new();
    let mut at_vertex: HashMap<Vertex, Caps> = HashMap::new();
    let mut capped = 0_usize;
    for (a, b, m) in fed(&planned) {
        let (ka, kb) = (key(a), key(b));
        let c = cap_key(m.feed_cap);
        capped += usize::from(m.feed_cap.is_some());
        by_ends
            .entry((ka.min(kb), ka.max(kb)))
            .or_default()
            .insert(c);
        at_vertex.entry(ka).or_default().insert(c);
        at_vertex.entry(kb).or_default().insert(c);
    }
    assert!(
        capped > 0,
        "the plan carries no cap; the census proves nothing"
    );

    let dressed = dress(&planned, &cutter);
    assert_no_feed_over_its_cap(&dressed, "the pipeline");
    let shipped = modulate(dressed);
    assert_no_feed_over_its_cap(&shipped, "the feed modulator");
    let (mut lines_read, mut capped_read, mut arcs_read) = (0_usize, 0_usize, 0_usize);
    for (i, (a, b, m)) in fed(&shipped).enumerate() {
        let (ka, kb) = (key(a), key(b));
        let c = cap_key(m.feed_cap);
        match m.move_type {
            MoveType::Linear { .. } => {
                // A line with two ends of one planned cut, where only one
                // cap meets those ends, is that cut.
                let Some(caps) = by_ends.get(&(ka.min(kb), ka.max(kb))) else {
                    continue;
                };
                if caps.len() != 1 {
                    continue;
                }
                lines_read += 1;
                capped_read += usize::from(m.feed_cap.is_some());
                assert!(
                    caps.contains(&c),
                    "fed line {i} ({a:?} to {b:?}): the cap {:?} is not the planned \
                     cut's {caps:?}",
                    m.feed_cap
                );
            }
            MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                let (Some(at_a), Some(at_b)) = (at_vertex.get(&ka), at_vertex.get(&kb)) else {
                    continue;
                };
                arcs_read += 1;
                capped_read += usize::from(m.feed_cap.is_some());
                assert!(
                    at_a.contains(&c) && at_b.contains(&c),
                    "fed arc {i} ({a:?} to {b:?}): the cap {:?} is not a planned cap at \
                     both ends ({at_a:?}, {at_b:?})",
                    m.feed_cap
                );
            }
            MoveType::Rapid => {}
        }
    }
    assert!(
        capped_read > 0 && lines_read + arcs_read > 0,
        "the census read no capped cut of the plan in the output \
         ({lines_read} lines, {arcs_read} arcs); it proves nothing"
    );
}
