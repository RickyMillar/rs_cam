//! Feeds contract P22b (`planning/feeds_contract_2026-11-03/phases/P22b.md`,
//! lead ruling section 3, option (a)): a Geodesic Adaptive op that states a
//! helix entry opens each layer piece with a helix bore; an op that states a
//! ramp keeps its ramp (ruling Q1).
//!
//! The fixture is the G-PHANTOMSTAMP boss (`adaptive3d_planner_never_ahead_of_emitted_path`:
//! a 60 x 60 plate, a 20 x 20 boss 12 mm tall with 45 degree walls, stock
//! top 15) cut by the 6 mm non-centre-cutting end mill of
//! `common/k6_project.rs` (core 2.4 mm, protrusion 0.5 mm), so the core
//! floor of the helix binds. The helix radius and pitch are the op's own
//! (`Adaptive3dConfig::helix_radius_for`, the G10 rule with the core floor;
//! the default pitch). No value of the rule is typed here.
//!
//! Helix op:
//! - every entry is a helix bore, or a ramp fallback with its
//!   `HelixCoreFallback` finding (one finding per ramp entry);
//! - no departure window: the planner models the bore, not a plunge
//!   (`plunge_departure_exempt` is false);
//! - the planner hole is the cut hole: the planner stock, stopped after each
//!   of the first levels and at the end, stands within the dexel read of a
//!   replay of the emitted path at every cell of the footprint.
//!
//! Ramp op (the Q1 side): every entry is a ramp, no entry is a helix, and the
//! departure windows stay.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

mod common;

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dOutput, Adaptive3dParams,
    Adaptive3dRuntimeEvent, ClearingStrategy3d, EntryStyle3d, FeedCapSource, RegionOrdering,
    adaptive_3d_toolpath_output_with_cancel, debug_adaptive_3d_segments_for_f029_probe,
};
use rs_cam_core::compute::operation_configs::Adaptive3dConfig;
use rs_cam_core::compute::tool_config::{
    CentreCutting, EntryFallback, HelixCoreFallback, HelixFallbackReason, NonCentreCuttingGeometry,
    ToolConfig, ToolFact, ToolId, ToolType,
};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::stock::radial_profile::LUT_SAMPLES;
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::tool::ToolDefinition;
use rs_cam_core::toolpath::MoveIntent;

// The boss fixture of `adaptive3d_planner_never_ahead_of_emitted_path.rs`.
const HALF: f64 = 30.0;
const BOSS_HALF: f64 = 10.0;
const BOSS_HEIGHT: f64 = 12.0;
const STOCK_TOP_Z: f64 = 15.0;
const SAFE_Z: f64 = 20.0;
const DPP: f64 = 2.6;
const STEPOVER: f64 = 2.2;
const LEAVE: f64 = 0.5;
const MIN_REGION_CUT_MM: f64 = 15.0;
/// The dexel read (`adaptive3d_planner_never_ahead_of_emitted_path`).
const READ_TOL_MM: f64 = 0.25;

fn boss_mesh() -> TriangleMesh {
    common::meshes::height_field(HALF, 0.5, |x, y| {
        let d = x.abs().max(y.abs());
        (BOSS_HEIGHT - (d - BOSS_HALF).max(0.0)).clamp(0.0, BOSS_HEIGHT)
    })
}

/// The 6 mm two-flute non-centre-cutting end mill of
/// `common/k6_project.rs::end_mill(true)`.
fn tool() -> ToolDefinition {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(20.0));
    tool.set_stickout(ToolFact::Stated(30.0));
    tool.set_shank_diameter(ToolFact::Stated(6.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool.set_centre_cutting(ToolFact::Stated(CentreCutting::No(
        NonCentreCuttingGeometry {
            centre_core_diameter_mm: 2.4,
            blade_protrusion_mm: 0.5,
        },
    )));
    rs_cam_core::compute::build_cutter(&tool).expect("the tool builds")
}

/// The op's helix entry and the op's ramp entry on `cutter`, from the
/// config defaults (no typed value).
fn style(helix: bool, cutter: &dyn MillingCutter) -> EntryStyle3d {
    let cfg = Adaptive3dConfig::default();
    if helix {
        EntryStyle3d::Helix {
            radius: cfg.helix_radius_for(cutter).emitted_mm,
            pitch: cfg.helix_pitch,
        }
    } else {
        EntryStyle3d::Ramp {
            max_angle_deg: cfg.ramp_angle_deg,
        }
    }
}

fn params(entry_style: EntryStyle3d, radius: f64, z_floor: Option<f64>) -> Adaptive3dParams {
    let cfg = Adaptive3dConfig::default();
    Adaptive3dParams {
        entry_clearance_mm: cfg.entry_clearance_mm,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: radius,
            envelope_radius: radius,
            stepover: STEPOVER,
            tolerance: 0.1,
            segment_merge_tolerance: None,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
            centre_boundary: Vec::new(),
            gouge_probe: None,
        },
        depth: Adaptive3dDepth {
            depth_per_pass: DPP,
            stock_to_leave: LEAVE,
            stock_top_z: STOCK_TOP_Z,
            z_floor,
            detect_flat_areas: false,
            rest_top_z: None,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global,
            min_region_cut_length_mm: MIN_REGION_CUT_MM,
            max_stay_down_distance_mm: None,
            stay_down_clearance_mm: 0.5,
            step_split: None,
            keep_down_route: None,
            level_contour: None,
            rest_region_filter: None,
        },
        trochoid_cap_mult: cfg.trochoid_cap_mult,
        engagement_measure: cfg.engagement_measure,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        safe_z: SAFE_Z,
        entry_style,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::GeodesicAdaptive,
    }
}

fn run(helix: bool, z_floor: Option<f64>) -> (Adaptive3dOutput, Adaptive3dParams) {
    let mesh = boss_mesh();
    let index = SpatialIndex::build(&mesh, 8.0);
    let cutter = tool();
    let p = params(style(helix, &cutter), cutter.radius(), z_floor);
    let out = adaptive_3d_toolpath_output_with_cancel(&mesh, &index, &cutter, &p, &|| false, None)
        .expect("planner");
    (out, p)
}

/// The style label of every entry of the run, in order.
fn entry_labels(out: &Adaptive3dOutput) -> Vec<&'static str> {
    entry_points(out)
        .into_iter()
        .map(|(_, label)| label)
        .collect()
}

/// The point and the style label of every entry of the run, in order.
fn entry_points(out: &Adaptive3dOutput) -> Vec<((f64, f64, f64), &'static str)> {
    out.annotations
        .iter()
        .filter_map(|a| {
            if let Adaptive3dRuntimeEvent::PassEntry {
                entry_x,
                entry_y,
                entry_z,
                style_label,
                ..
            } = a.event
            {
                Some(((entry_x, entry_y, entry_z), style_label))
            } else {
                None
            }
        })
        .collect()
}

/// The capped runs whose source is a departure window.
fn departure_windows(out: &Adaptive3dOutput) -> usize {
    out.annotations
        .iter()
        .filter(|a| {
            matches!(
                a.event,
                Adaptive3dRuntimeEvent::FeedCapStart {
                    source: FeedCapSource::DepartureWindow,
                    ..
                }
            )
        })
        .count()
}

/// The Z levels of the plan, so a `z_floor` at one stops the plan there.
fn level_zs() -> Vec<f64> {
    let mut out = Vec::new();
    let mut z = STOCK_TOP_Z - DPP;
    while z > LEAVE {
        out.push(z);
        z -= DPP;
    }
    out
}

/// The move ranges `[first, end)` of the layer helix bores of the run: a
/// "helix entry" whose last marker is a level marker, not the waterline
/// cleanup (its entries keep the op's own helix, outside P22b).
fn layer_bore_ranges(out: &Adaptive3dOutput) -> Vec<(usize, usize)> {
    let mut in_waterline = false;
    let mut ranges = Vec::new();
    for a in &out.annotations {
        match a.event {
            Adaptive3dRuntimeEvent::WaterlineCleanup => in_waterline = true,
            Adaptive3dRuntimeEvent::GlobalZLevel { .. }
            | Adaptive3dRuntimeEvent::RegionZLevel { .. } => in_waterline = false,
            Adaptive3dRuntimeEvent::PassEntry {
                entry_end_move_idx,
                style_label,
                ..
            } if style_label == "helix entry" && !in_waterline => {
                ranges.push((a.move_index, entry_end_move_idx));
            }
            Adaptive3dRuntimeEvent::RegionStart { .. }
            | Adaptive3dRuntimeEvent::PassEntry { .. }
            | Adaptive3dRuntimeEvent::PassPreflightSkip { .. }
            | Adaptive3dRuntimeEvent::PassSummary { .. }
            | Adaptive3dRuntimeEvent::FeedCapStart { .. }
            | Adaptive3dRuntimeEvent::FeedCapEnd => {}
        }
    }
    ranges
}

/// The planner stock against a replay of the emitted path (the cut table,
/// as the simulation stamps a non-centre-cutting tool), over the footprint:
/// the cells where the planner stands below the replay by more than the
/// dexel read (ahead of the cut), and the cells where it stands above,
/// keyed by what first cut them below the planner (`layer bore` for a move
/// of a layer helix bore, else the move intent).
fn plan_cut_gap(
    helix: bool,
    z_floor: Option<f64>,
) -> (usize, std::collections::BTreeMap<String, usize>) {
    let (out, p) = run(helix, z_floor);
    let mesh = boss_mesh();
    let index = SpatialIndex::build(&mesh, 8.0);
    let cutter = tool();
    let (plan_tops, rows, cols, cell, ou, ov, zmin, zmax) =
        debug_adaptive_3d_segments_for_f029_probe(&mesh, &index, &cutter, &p, &|| false)
            .expect("planner probe");
    let b = mesh.bbox;
    let r = cutter.radius();
    let fresh = || {
        TriDexelStock::from_stock(
            b.min.x - r,
            b.min.y - r,
            b.max.x + r,
            b.max.y + r,
            zmin,
            zmax,
            cell,
        )
    };
    let lut = rs_cam_core::tool::body::cut_table(&cutter, LUT_SAMPLES);
    let mut replay = fresh();
    assert_eq!((replay.z_grid.rows, replay.z_grid.cols), (rows, cols));
    let moves = &out.toolpath.moves;
    for w in moves.windows(2) {
        replay.stamp_linear_segment(
            &lut,
            r,
            w[0].target,
            w[1].target,
            StockCutDirection::FromTop,
        );
    }
    let plan_at = |row: usize, c: usize| {
        let plan = f64::from(plan_tops[row * cols + c]);
        if plan.is_finite() { plan } else { zmin }
    };
    let mut ahead = 0usize;
    let mut behind: Vec<(usize, usize)> = Vec::new();
    for row in 0..rows {
        for c in 0..cols {
            let (x, y) = (ou + c as f64 * cell, ov + row as f64 * cell);
            if x.abs() > HALF || y.abs() > HALF {
                continue;
            }
            let gap = replay.z_grid.top_z_at(row, c).map_or(zmin, f64::from) - plan_at(row, c);
            if gap > READ_TOL_MM {
                ahead += 1;
            } else if -gap > READ_TOL_MM {
                behind.push((row, c));
            }
        }
    }
    // The move that first cut each behind cell below the planner.
    let bores = layer_bore_ranges(&out);
    let mut by_cause = std::collections::BTreeMap::new();
    let mut open = behind;
    let mut again = fresh();
    for (k, w) in moves.windows(2).enumerate() {
        if open.is_empty() {
            break;
        }
        again.stamp_linear_segment(
            &lut,
            r,
            w[0].target,
            w[1].target,
            StockCutDirection::FromTop,
        );
        let m = k + 1;
        open.retain(|&(row, c)| {
            let now = again.z_grid.top_z_at(row, c).map_or(zmin, f64::from);
            if plan_at(row, c) - now <= READ_TOL_MM {
                return true;
            }
            // The bore range starts at the retract and the rapid before
            // the helix; only the helix moves (`EntryHelix`) cut the hole.
            let helix_move = matches!(moves[m].intent, MoveIntent::EntryHelix);
            let cause = if helix_move && bores.iter().any(|&(a, e)| m >= a && m < e) {
                "layer bore".to_owned()
            } else {
                format!("{:?}", moves[m].intent)
            };
            *by_cause.entry(cause).or_insert(0usize) += 1;
            false
        });
    }
    (ahead, by_cause)
}

#[test]
fn a_geodesic_layer_opens_with_a_helix_bore_p22b() {
    let (out, _) = run(true, None);
    let labels = entry_labels(&out);
    let helix = labels.iter().filter(|l| **l == "helix entry").count();
    let ramp = labels.iter().filter(|l| **l == "ramp entry").count();
    let other: Vec<_> = labels
        .iter()
        .filter(|l| !matches!(**l, "helix entry" | "ramp entry" | "keep-down link"))
        .collect();
    let fallbacks = out.produced.helix_core_fallbacks.len();
    let windows = departure_windows(&out);
    eprintln!(
        "helix op: entries {}, helix {helix}, ramp fallback {ramp}, findings {fallbacks}, \
         departure windows {windows}",
        labels.len()
    );
    assert!(helix > 0, "non-vacuity: the helix op has helix entries");
    assert!(
        other.is_empty(),
        "every entry is a bore or a fallback: {other:?}"
    );
    // Each fallback names an entry, and each entry that ramps has a
    // fallback (a keep-down link ends above the stock and the ramp then
    // takes the material, so a linked entry can fall back too).
    let entries = entry_points(&out);
    let at = |p: (f64, f64, f64), q: (f64, f64, f64)| {
        (p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9 && (p.2 - q.2).abs() < 1e-9
    };
    let found: Vec<(f64, f64, f64)> = out
        .produced
        .helix_core_fallbacks
        .iter()
        .map(|f| (f.at.x, f.at.y, f.at.z))
        .collect();
    assert!(
        found
            .iter()
            .all(|f| entries.iter().any(|(e, _)| at(*f, *e))),
        "every HelixCoreFallback names an entry"
    );
    assert!(
        entries
            .iter()
            .filter(|(_, label)| *label == "ramp entry")
            .all(|(e, _)| found.iter().any(|f| at(*f, *e))),
        "every ramp entry of the helix op has its HelixCoreFallback"
    );
    assert!(
        out.produced
            .helix_core_fallbacks
            .iter()
            .all(|f| f.fit_mm < f.core_floor_mm),
        "a fallback is a fit under the core floor"
    );
    // Two reasons, one finding id: a fit of 0 is "no room for the helix",
    // a fit above 0 under the core floor is "helix radius below the core
    // radius" (`HelixFallbackReason`).
    let (no_room, below_core): (Vec<&HelixCoreFallback>, Vec<&HelixCoreFallback>) = out
        .produced
        .helix_core_fallbacks
        .iter()
        .partition(|f| f.reason() == HelixFallbackReason::NoRoom);
    eprintln!(
        "helix op fallbacks: {} {:?}, {} {:?}",
        no_room.len(),
        HelixFallbackReason::NoRoom.text(),
        below_core.len(),
        HelixFallbackReason::BelowCoreRadius.text()
    );
    // The reason reads the fit by the dressup's room test: every "no room"
    // fit lies under every "below the core radius" fit, and both reasons
    // occur on this fixture.
    let widest_no_room = no_room.iter().map(|f| f.fit_mm).fold(0.0, f64::max);
    assert!(below_core.iter().all(|f| f.fit_mm > widest_no_room));
    assert!(!no_room.is_empty() && !below_core.is_empty());
    // The lead's wanaka case (P22f_wanaka.md 9.3): after a helix refusal on
    // a non-centre-cutting tool the entry does not plunge straight into the
    // stock. The fallback is the ramp at the limit angle; no move of the op
    // is a plunge, and no entry needed the G-CENTRECUT plunge refusal.
    assert!(
        out.produced
            .helix_core_fallbacks
            .iter()
            .all(|f| f.fallback == EntryFallback::Ramp),
        "every fallback is the ramp"
    );
    assert!(
        !out.toolpath
            .moves
            .iter()
            .any(|m| matches!(m.intent, MoveIntent::EntryPlunge)),
        "no straight plunge after a helix refusal"
    );
    assert!(
        out.produced.non_centre_cutting_refusal.is_none(),
        "no entry plunged deeper than the protrusion: {:?}",
        out.produced.non_centre_cutting_refusal
    );
    assert_eq!(windows, 0, "a bore leaves no departure window");

    assert!(
        !layer_bore_ranges(&out).is_empty(),
        "non-vacuity: the run has layer bores"
    );

    let mut floors: Vec<Option<f64>> = level_zs().into_iter().take(2).map(Some).collect();
    floors.push(None);
    let mut failures = Vec::new();
    for z_floor in floors {
        let (ahead, behind) = plan_cut_gap(true, z_floor);
        eprintln!(
            "helix op, z_floor {z_floor:?}: planner ahead of the cut at {ahead} cells; behind, \
             by what first cut the cell: {behind:?}"
        );
        if ahead > 0 || behind.contains_key("layer bore") {
            failures.push((z_floor, ahead, behind));
        }
    }
    assert!(
        failures.is_empty(),
        "the planner hole is the cut hole: no cell ahead of the cut, and no cell a layer bore \
         cut beyond the planner (z_floor, ahead, behind by cause): {failures:?}"
    );
}

#[test]
fn a_ramp_op_keeps_its_ramp_q1_p22b() {
    let (out, p) = run(false, None);
    let labels = entry_labels(&out);
    let windows = departure_windows(&out);
    eprintln!(
        "ramp op: entries {}, labels {:?}, departure windows {windows}",
        labels.len(),
        {
            let mut l = labels.clone();
            l.sort_unstable();
            l.dedup();
            l
        }
    );
    assert!(
        labels.contains(&"ramp entry"),
        "non-vacuity: the ramp op has ramp entries"
    );
    assert!(
        !labels.contains(&"helix entry"),
        "a ramp op cuts no helix (ruling Q1)"
    );
    assert!(out.produced.helix_core_fallbacks.is_empty());
    assert!(
        windows > 0,
        "the ramp op keeps its departure windows (the bore is a helix-op rule)"
    );
    assert!(matches!(p.entry_style, EntryStyle3d::Ramp { .. }));

    // The candidate P22b-2 (lead ruling point 4): the planner stamps a disc
    // of R at a ramp entry while the ramp cuts a slot. That errs on the
    // conservative side: the planner stands above the cut (it reads stock
    // where the cut left air), never below it.
    let (ahead, behind) = plan_cut_gap(false, None);
    eprintln!(
        "ramp op: planner ahead of the cut at {ahead} cells; behind, by what first cut the \
         cell: {behind:?}"
    );
    assert_eq!(ahead, 0, "the planner is never ahead of a ramp entry's cut");
}
