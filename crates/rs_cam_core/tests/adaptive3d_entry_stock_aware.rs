//! Stock-aware adaptive3d entries — the safety sentry for
//! `planning/entry_stock_awareness_2026-09-24/PLAN.md`.
//!
//! ## What this file holds
//!
//! The rough emits its entries from the planner's own dexel stock: a rapid
//! stops above the real local material (option 1), and a keep-down link
//! runs only through a corridor that the planner stock shows clear (option
//! 2). This file replays the EMITTED path on a fresh dexel stock, move by
//! move, and measures every move against the material that stands at that
//! moment. The oracle is the replay, not the planner, so a planner that
//! believes a wrong stock cannot pass it.
//!
//! ## The fixture
//!
//! A flat plate at Z 0 under 16 mm of prism stock, a 6 mm flat end mill,
//! Depth/Pass 8, Contour Parallel. Two levels (Z 8 and Z 0). Before the
//! fix, plunge style with the default keep-down (8 x D) took the last cut
//! of level 8 to the first ring of level 0 with a fed straight descent of
//! 8 mm into uncut stock: the probe's "8 mm plunge" on rivmap100
//! (`planning/adaptive3d_step_ladder_roughing_2026-09-24/VALLEY_ENTRY_PROBE.md`).
//!
//! ## The rules the replay holds
//!
//! - No rapid enters material.
//! - A `Linking` feed that descends steeply goes through air only.
//! - A fed steep descent of any other intent enters at most one peck
//!   (Depth/Pass) of material.
//! - The finished stock is the same for every style: the plate is clear.
//!
//! CAUTION, a known limit that this file does not close: `dressup::emit_helix`
//! and `dressup::emit_ramp` helix or ramp only the last `ENTRY_CLEARANCE`
//! (2 mm). Above that they feed a straight `EntryPlunge` through the material
//! (6.0 mm of the 8 mm here). The emitters are shared with the 2.5D dressup
//! door, so the change needs an operator ruling. The bound for helix and ramp
//! styles is therefore the peck bound, not the helix pitch or the ramp angle.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, RegionOrdering, adaptive_3d_toolpath,
};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_flat, make_test_hemisphere};
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

const PLATE: f64 = 40.0;
const STOCK_TOP_Z: f64 = 16.0;
const SAFE_Z: f64 = 21.0;
const DPP: f64 = 8.0;
const CELL: f64 = 0.25;
/// The replay reads material a little inside the tool footprint, so a
/// dexel cell on the footprint edge is not read as a bite.
const PROBE_INSET: f64 = 2.0 * CELL;
/// Dexel noise on a depth reading.
const DEPTH_TOL: f64 = 0.25;

fn params(style: EntryStyle3d, stay_down: Option<f64>, dpp: f64) -> Adaptive3dParams {
    Adaptive3dParams {
        geometry: Adaptive3dGeometry {
            tool_radius: 3.0,
            envelope_radius: 3.0,
            stepover: 1.8,
            tolerance: 0.1,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
        },
        depth: Adaptive3dDepth {
            depth_per_pass: dpp,
            stock_to_leave: 0.0,
            stock_top_z: STOCK_TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::ByArea,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: stay_down,
            stay_down_clearance_mm: 0.5,
        },
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        feed_rate: 2400.0,
        plunge_rate: 500.0,
        safe_z: SAFE_Z,
        entry_style: style,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::ContourParallel,
        z_blend: false,
    }
}

fn generate(style: EntryStyle3d, stay_down: Option<f64>) -> (Toolpath, FlatEndmill) {
    generate_on(&make_test_flat(PLATE), style, stay_down, DPP)
}

fn generate_on(
    mesh: &TriangleMesh,
    style: EntryStyle3d,
    stay_down: Option<f64>,
    dpp: f64,
) -> (Toolpath, FlatEndmill) {
    let index = SpatialIndex::build(mesh, 8.0);
    let tool = FlatEndmill::new(6.0, 25.0);
    let tp = adaptive_3d_toolpath(mesh, &index, &tool, &params(style, stay_down, dpp));
    assert!(!tp.moves.is_empty(), "the rough emitted nothing");
    (tp, tool)
}

/// The prism stock the planner builds: the mesh XY box up to the stock top.
fn seed_stock(half: f64) -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(-half, -half, -2.0),
        max: P3::new(half, half, STOCK_TOP_Z),
    };
    TriDexelStock::from_bounds(&bbox, CELL)
}

/// One steep fed descent, measured against the replayed stock.
#[derive(Debug)]
struct Descent {
    move_index: usize,
    intent: MoveIntent,
    /// Material the descent went into (mm), under the tool footprint.
    depth: f64,
}

#[derive(Debug, Default)]
struct Audit {
    /// (move index, depth of material the rapid went into).
    rapid_hits: Vec<(usize, f64)>,
    descents: Vec<Descent>,
    /// Highest material left over the plate, inside one tool radius.
    final_max_top: f64,
    retracts_to_safe: usize,
}

fn replay(tp: &Toolpath, tool: &FlatEndmill) -> Audit {
    replay_on(tp, tool, PLATE / 2.0)
}

fn replay_on(tp: &Toolpath, tool: &FlatEndmill, h: f64) -> Audit {
    let r = tool.radius();
    let mut stock = seed_stock(h);
    let lut = RadialProfileLUT::from_cutter(tool, LUT_SAMPLES);
    let probe_r = r - PROBE_INSET;
    let top_at = |stock: &TriDexelStock, x: f64, y: f64| -> f64 {
        stock
            .max_top_z_in_disc(x, y, probe_r)
            .unwrap_or(f64::NEG_INFINITY)
    };
    let mut audit = Audit::default();
    for i in 1..tp.moves.len() {
        let a = tp.moves[i - 1].target;
        let m = &tp.moves[i];
        let b = m.target;
        let (dx, dy, dz) = (b.x - a.x, b.y - a.y, a.z - b.z);
        let xy = (dx * dx + dy * dy).sqrt();
        match m.move_type {
            MoveType::Rapid => {
                if b.z >= SAFE_Z - 1e-6 && a.z < SAFE_Z - 1e-6 {
                    audit.retracts_to_safe += 1;
                }
                let len = (xy * xy + dz * dz).sqrt();
                let n = (len / CELL).ceil().max(1.0) as usize;
                let mut worst = 0.0f64;
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    let p = P3::new(a.x + dx * t, a.y + dy * t, a.z - dz * t);
                    worst = worst.max(top_at(&stock, p.x, p.y) - p.z);
                }
                if worst > DEPTH_TOL {
                    audit.rapid_hits.push((i, worst));
                }
            }
            // Steep: more than 60 degrees below the horizontal.
            MoveType::Linear { .. } if dz > 0.05 && xy < dz * 0.577 => {
                let top = top_at(&stock, b.x, b.y).min(a.z);
                audit.descents.push(Descent {
                    move_index: i,
                    intent: m.intent,
                    depth: (top - b.z).max(0.0),
                });
            }
            _ => {}
        }
        stock.stamp_linear_segment(&lut, r, a, b, StockCutDirection::FromTop);
    }
    let inner = h - r;
    let mut max_top = f64::NEG_INFINITY;
    let mut y = -inner;
    while y <= inner {
        let mut x = -inner;
        while x <= inner {
            max_top = max_top.max(top_at(&stock, x, y));
            x += 1.0;
        }
        y += 1.0;
    }
    audit.final_max_top = max_top;
    audit
}

fn deepest(audit: &Audit, pick: impl Fn(MoveIntent) -> bool) -> Option<&Descent> {
    audit
        .descents
        .iter()
        .filter(|d| pick(d.intent))
        .max_by(|a, b| a.depth.total_cmp(&b.depth))
}

fn check_safe(label: &str, audit: &Audit) {
    check_safe_dpp(label, audit, DPP, true);
}

fn check_safe_dpp(label: &str, audit: &Audit, dpp: f64, plate_clear: bool) {
    let link = deepest(audit, |i| i == MoveIntent::Linking);
    let other = deepest(audit, |i| i != MoveIntent::Linking);
    eprintln!(
        "{label}: rapid hits {}, deepest link descent {:?}, deepest other descent {:?}, \
         retracts {}, final max top {:.3}",
        audit.rapid_hits.len(),
        link.map(|d| (d.move_index, d.depth)),
        other.map(|d| (d.move_index, d.intent, d.depth)),
        audit.retracts_to_safe,
        audit.final_max_top,
    );
    assert!(
        audit.rapid_hits.is_empty(),
        "{label}: rapids enter material: {:?}",
        audit.rapid_hits
    );
    if let Some(d) = link {
        assert!(
            d.depth <= DEPTH_TOL,
            "{label}: a Linking feed at move {} goes {:.2} mm straight down into stock",
            d.move_index,
            d.depth
        );
    }
    if let Some(d) = other {
        assert!(
            d.depth <= dpp + DEPTH_TOL,
            "{label}: a {:?} descent at move {} goes {:.2} mm into stock, more than one peck",
            d.intent,
            d.move_index,
            d.depth
        );
    }
    assert!(
        !plate_clear || audit.final_max_top <= DEPTH_TOL,
        "{label}: the plate is not clear (max top {:.3})",
        audit.final_max_top
    );
}

/// The probe's case: plunge style with the default keep-down (8 x D).
#[test]
fn plunge_keep_down_never_feeds_straight_into_stock() {
    let (tp, tool) = generate(EntryStyle3d::Plunge, None);
    check_safe("plunge, keep-down 8D", &replay(&tp, &tool));
}

/// Option 2 fires: with the keep-down on, the rough retracts to safe Z
/// fewer times than with it off, and both are safe.
#[test]
fn keep_down_saves_retracts_on_every_style() {
    let styles = [
        ("plunge", EntryStyle3d::Plunge),
        (
            "helix",
            EntryStyle3d::Helix {
                radius: 1.8,
                pitch: 1.0,
            },
        ),
        ("ramp", EntryStyle3d::Ramp { max_angle_deg: 3.0 }),
    ];
    for (name, style) in styles {
        let (tp_on, tool) = generate(style, None);
        let (tp_off, _) = generate(style, Some(0.0));
        let on = replay(&tp_on, &tool);
        let off = replay(&tp_off, &tool);
        check_safe(&format!("{name}, keep-down on"), &on);
        check_safe(&format!("{name}, keep-down off"), &off);
        assert!(
            on.retracts_to_safe < off.retracts_to_safe,
            "{name}: the keep-down link did not save a retract ({} on, {} off)",
            on.retracts_to_safe,
            off.retracts_to_safe
        );
    }
}

/// A dome under prism stock gives contour rings with walls beside them: the
/// ring starts, the side entries at depth and the keep-down links over
/// uncut shoulders. Every style, keep-down on, Depth/Pass 4.
#[test]
fn dome_entries_are_stock_safe_on_every_style() {
    let dome = make_test_hemisphere(15.0, 24);
    let styles = [
        ("plunge", EntryStyle3d::Plunge),
        (
            "helix",
            EntryStyle3d::Helix {
                radius: 1.8,
                pitch: 1.0,
            },
        ),
        ("ramp", EntryStyle3d::Ramp { max_angle_deg: 3.0 }),
    ];
    for (name, style) in styles {
        let (tp, tool) = generate_on(&dome, style, None, 4.0);
        let audit = replay_on(&tp, &tool, 15.0);
        check_safe_dpp(&format!("dome, {name}"), &audit, 4.0, false);
    }
}

/// `session/compute.rs` runs `optimize_entry_descents_annotated` on this
/// output with the op's SEED stock. It must not change it: its split target
/// (seed stock top + clearance) is never below where the planner's rapid
/// already stops, so it cannot rapid below material the planner reads.
#[test]
fn entry_optimiser_leaves_the_planner_entries_alone() {
    for style in [
        EntryStyle3d::Plunge,
        EntryStyle3d::Helix {
            radius: 1.8,
            pitch: 1.0,
        },
    ] {
        let (tp, tool) = generate(style, None);
        let mut optimised = tp.clone();
        let stock = seed_stock(PLATE / 2.0);
        let splits = rs_cam_core::dressup::optimize_entry_descents(
            &mut optimised,
            Some(&stock),
            STOCK_TOP_Z,
            tool.radius(),
            &tool,
            None,
        );
        assert_eq!(splits, 0, "{style:?}: the optimiser split a planner entry");
        assert_eq!(optimised.moves.len(), tp.moves.len());
    }
}
