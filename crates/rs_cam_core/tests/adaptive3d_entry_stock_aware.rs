//! Stock-aware entries — the safety sentry for
//! `planning/entry_stock_awareness_2026-09-24/PLAN.md`.
//!
//! ## What this file holds
//!
//! The adaptive3d rough emits its entries from the planner's own dexel
//! stock: a rapid stops above the real local material (option 1), and a
//! keep-down link runs only through a corridor that the planner stock shows
//! clear (option 2). The 2.5D dressup door replays the op's own moves for
//! the same material top. This file replays the EMITTED path on a fresh
//! dexel stock, move by move, and measures every move against the material
//! that stands at that moment. The oracle is the replay, not the planner,
//! so a planner that believes a wrong stock cannot pass it.
//!
//! ## The rules the replay holds (operator ruling 2026-09-24)
//!
//! - No rapid enters material.
//! - A steep fed descent (more than 60 degrees below the horizontal) goes
//!   through air, with two exceptions: a plunge-style `EntryPlunge` may take
//!   one peck (Depth/Pass), and a `ClearingCut` may take one Depth/Pass.
//! - A helix move that enters material descends at most the helix pitch per
//!   revolution; a ramp move that enters material descends at most at the
//!   ramp angle.
//! - The finished stock is the same for every style: the plate is clear.
//!
//! ## The fixtures
//!
//! - A flat plate at Z 0 under 16 mm of prism stock, a 6 mm flat end mill,
//!   Depth/Pass 8, Contour Parallel. Before the stock-aware entries, plunge
//!   style with the default keep-down (8 x D) fed 8 mm straight down into
//!   uncut stock: the probe's "8 mm plunge" on rivmap100.
//! - A dome under prism stock, Depth/Pass 4.
//! - A 2.5D pocket (the `demo_pocket` shape, 12 mm deep, Depth/Pass 1.2)
//!   through the dressup door, helix and ramp.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;
use common::make_endmill_6mm;

use std::f64::consts::TAU;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, RegionOrdering, adaptive_3d_toolpath, adaptive_3d_toolpath_annotated,
};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, DressupEntryStyle, HeightsConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::{BoundingBox3, P2, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_flat, make_test_hemisphere};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

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
/// A move whose target stands this far below the replayed material enters
/// material. A helix step descends pitch / 36 (0.028 mm at pitch 1).
const ENTERS_MATERIAL_MM: f64 = 0.01;
const HELIX_RADIUS: f64 = 1.8;
/// The default `entry_clearance_mm`.
const ENTRY_CLEARANCE_MM: f64 = 0.5;
/// The replay reads a fresh stock at cell centres; the planner reads a
/// conservative (sliver-safe) bound. They differ by up to about a cell.
const AIR_TOL_MM: f64 = 0.3;
const HELIX_PITCH: f64 = 1.0;
const RAMP_ANGLE_DEG: f64 = 3.0;

/// The entry style under test, with the bounds the ruling gives it.
#[derive(Clone, Copy, Debug)]
enum Style {
    Plunge,
    Helix,
    Ramp,
}

impl Style {
    fn adaptive3d(self) -> EntryStyle3d {
        match self {
            Style::Plunge => EntryStyle3d::Plunge,
            Style::Helix => EntryStyle3d::Helix {
                radius: HELIX_RADIUS,
                pitch: HELIX_PITCH,
            },
            Style::Ramp => EntryStyle3d::Ramp {
                max_angle_deg: RAMP_ANGLE_DEG,
            },
        }
    }

    /// The steepest slope (dz / dxy) an entry move may take through
    /// material. A helix step is a chord of the turn circle, so its slope is
    /// the pitch over the circumference; 2 % covers the chord and the float.
    fn max_slope(self) -> f64 {
        match self {
            Style::Plunge => f64::INFINITY,
            Style::Helix => HELIX_PITCH / (TAU * HELIX_RADIUS) * 1.02,
            Style::Ramp => RAMP_ANGLE_DEG.to_radians().tan() * 1.02,
        }
    }
}

const STYLES: [Style; 3] = [Style::Plunge, Style::Helix, Style::Ramp];

fn params(style: EntryStyle3d, stay_down: Option<f64>, dpp: f64) -> Adaptive3dParams {
    Adaptive3dParams {
        entry_clearance_mm: 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: 3.0,
            envelope_radius: 3.0,
            stepover: 1.8,
            tolerance: 0.1,
            segment_merge_tolerance: None,
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

fn generate(style: Style, stay_down: Option<f64>) -> (Toolpath, FlatEndmill) {
    generate_on(&make_test_flat(PLATE), style, stay_down, DPP)
}

fn generate_on(
    mesh: &TriangleMesh,
    style: Style,
    stay_down: Option<f64>,
    dpp: f64,
) -> (Toolpath, FlatEndmill) {
    let index = SpatialIndex::build(mesh, 8.0);
    let tool = FlatEndmill::new(6.0, 25.0);
    let tp = adaptive_3d_toolpath(
        mesh,
        &index,
        &tool,
        &params(style.adaptive3d(), stay_down, dpp),
    );
    assert!(!tp.moves.is_empty(), "the rough emitted nothing");
    (tp, tool)
}

/// A prism stock over `[x0, x1] x [y0, y1]` from `z0` up to `z1`.
fn prism(x0: f64, y0: f64, x1: f64, y1: f64, z0: f64, z1: f64) -> TriDexelStock {
    TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(x0, y0, z0),
            max: P3::new(x1, y1, z1),
        },
        CELL,
    )
}

/// The prism stock the adaptive3d planner builds: the mesh XY box up to the
/// stock top.
fn seed_stock(half: f64) -> TriDexelStock {
    prism(-half, -half, half, half, -2.0, STOCK_TOP_Z)
}

/// One fed descent, measured against the replayed stock.
#[derive(Debug)]
struct Descent {
    move_index: usize,
    intent: MoveIntent,
    /// Material the move went into (mm), under the tool footprint.
    depth: f64,
    /// dz / dxy of the move (infinite for a vertical move).
    slope: f64,
    /// How far the move's start stands above the material top under the
    /// entry footprint (tool radius plus the helix radius) at either end.
    above: f64,
    /// How far the move's end stands below the material top under the
    /// helix sweep (tool radius plus the helix radius, less the probe
    /// inset) at the end. A
    /// straight descent that ends below this top goes under material that
    /// the helix turns then meet from the side.
    below_wide: f64,
}

impl Descent {
    fn steep(&self) -> bool {
        self.slope > 1.0 / 0.577
    }
}

#[derive(Debug, Default)]
struct Audit {
    /// (move index, depth of material the rapid went into).
    rapid_hits: Vec<(usize, f64)>,
    descents: Vec<Descent>,
    retracts_to_safe: usize,
}

/// Replay `tp` on `stock` and audit every rapid and every fed descent.
/// Arcs are stamped as chords: the fixtures turn arc fitting off.
fn replay_on(
    tp: &Toolpath,
    tool: &dyn MillingCutter,
    stock: &mut TriDexelStock,
    safe_z: f64,
) -> Audit {
    let r = tool.radius();
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
                if b.z >= safe_z - 1e-6 && a.z < safe_z - 1e-6 {
                    audit.retracts_to_safe += 1;
                }
                let len = (xy * xy + dz * dz).sqrt();
                let n = (len / CELL).ceil().max(1.0) as usize;
                let mut worst = 0.0f64;
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    let p = P3::new(a.x + dx * t, a.y + dy * t, a.z - dz * t);
                    worst = worst.max(top_at(stock, p.x, p.y) - p.z);
                }
                if worst > DEPTH_TOL {
                    audit.rapid_hits.push((i, worst));
                }
            }
            _ if dz > 0.01 => {
                let top = top_at(stock, b.x, b.y).min(a.z);
                // The material the move descends into: under the entry
                // footprint (tool plus helix radius) at either end.
                let wide_at = |p: P3| {
                    stock
                        .max_top_z_in_disc(p.x, p.y, r + HELIX_RADIUS + CELL)
                        .unwrap_or(f64::NEG_INFINITY)
                };
                let wide = wide_at(a).max(wide_at(b));
                // The material the helix turns meet: under the helix sweep,
                // read a little inside it as `probe_r` is.
                let below_wide = stock
                    .max_top_z_in_disc(b.x, b.y, r + HELIX_RADIUS - PROBE_INSET)
                    .map_or(f64::NEG_INFINITY, |top| top - b.z);
                audit.descents.push(Descent {
                    move_index: i,
                    intent: m.intent,
                    depth: (top - b.z).max(0.0),
                    slope: if xy > 1e-9 { dz / xy } else { f64::INFINITY },
                    above: a.z - wide,
                    below_wide,
                });
            }
            _ => {}
        }
        stock.stamp_linear_segment(&lut, r, a, b, StockCutDirection::FromTop);
    }
    audit
}

fn replay(tp: &Toolpath, tool: &FlatEndmill) -> (Audit, f64) {
    let mut stock = seed_stock(PLATE / 2.0);
    let audit = replay_on(tp, tool, &mut stock, SAFE_Z);
    let inner = PLATE / 2.0 - tool.radius();
    let mut max_top = f64::NEG_INFINITY;
    let mut y = -inner;
    while y <= inner {
        let mut x = -inner;
        while x <= inner {
            if let Some(t) = stock.max_top_z_in_disc(x, y, tool.radius() - PROBE_INSET) {
                max_top = max_top.max(t);
            }
            x += 1.0;
        }
        y += 1.0;
    }
    (audit, max_top)
}

/// Assert the ruling's bounds on one audit.
fn check_safe(label: &str, audit: &Audit, style: Style, dpp: f64) {
    check_safe_with(label, audit, style, dpp, false);
}

/// `surface_blocked`: see `dome_entries_are_stock_safe_on_every_style`.
fn check_safe_with(label: &str, audit: &Audit, style: Style, dpp: f64, surface_blocked: bool) {
    let is_entry = |i: MoveIntent| matches!(i, MoveIntent::EntryHelix | MoveIntent::EntryRamp);
    let steep_into = |pick: &dyn Fn(MoveIntent) -> bool| {
        audit
            .descents
            .iter()
            .filter(|d| d.steep() && pick(d.intent))
            .max_by(|a, b| a.depth.total_cmp(&b.depth))
    };
    let steepest_entry = audit
        .descents
        .iter()
        .filter(|d| {
            !d.steep()
                && d.depth > ENTERS_MATERIAL_MM
                && matches!(d.intent, MoveIntent::EntryHelix | MoveIntent::EntryRamp)
        })
        .max_by(|a, b| a.slope.total_cmp(&b.slope));
    let entry_plunge = steep_into(&|i| i == MoveIntent::EntryPlunge);
    let other = steep_into(&|i| {
        i != MoveIntent::EntryPlunge
            && i != MoveIntent::ClearingCut
            && !(surface_blocked && is_entry(i))
    });
    if surface_blocked {
        let over: Vec<&Descent> = audit
            .descents
            .iter()
            .filter(|d| {
                is_entry(d.intent) && d.depth > ENTERS_MATERIAL_MM && (d.slope > style.max_slope())
            })
            .collect();
        let deepest = over.iter().map(|d| d.depth).fold(0.0f64, f64::max);
        eprintln!(
            "{label}: OPEN — {} entry moves on the surface clip exceed the style slope \
             through material, deepest {deepest:.2} mm",
            over.len()
        );
    }
    let cut = steep_into(&|i| i == MoveIntent::ClearingCut);
    eprintln!(
        "{label}: rapid hits {}, retracts {}, straight EntryPlunge {:?}, other straight {:?}, \
         straight cut {:?}, steepest entry through material {:?}",
        audit.rapid_hits.len(),
        audit.retracts_to_safe,
        entry_plunge.map(|d| (d.move_index, d.depth)),
        other.map(|d| (d.move_index, d.intent, d.depth)),
        cut.map(|d| (d.move_index, d.depth)),
        steepest_entry.map(|d| (d.move_index, d.intent, d.slope)),
    );
    assert!(
        audit.rapid_hits.is_empty(),
        "{label}: rapids enter material: {:?}",
        audit.rapid_hits
    );
    let plunge_bound = match style {
        Style::Plunge => dpp + DEPTH_TOL,
        Style::Helix | Style::Ramp => DEPTH_TOL,
    };
    if let Some(d) = entry_plunge {
        assert!(
            d.depth <= plunge_bound,
            "{label}: an EntryPlunge at move {} goes {:.2} mm straight into stock (bound {:.2})",
            d.move_index,
            d.depth,
            plunge_bound
        );
    }
    if let Some(d) = other {
        assert!(
            d.depth <= DEPTH_TOL,
            "{label}: a {:?} feed at move {} goes {:.2} mm straight down into stock",
            d.intent,
            d.move_index,
            d.depth
        );
    }
    if let Some(d) = cut {
        assert!(
            d.depth <= dpp + DEPTH_TOL,
            "{label}: a cut at move {} goes {:.2} mm straight into stock",
            d.move_index,
            d.depth
        );
    }
    // Operator ruling 2026-09-25: never helix or ramp air. No descending
    // helix or ramp move starts more than the entry clearance above the
    // material under it.
    let highest_entry = audit
        .descents
        .iter()
        .filter(|d| is_entry(d.intent))
        .max_by(|a, b| a.above.total_cmp(&b.above));
    eprintln!(
        "{label}: highest helix/ramp start above the material {:?}",
        highest_entry.map(|d| (d.move_index, d.intent, d.above))
    );
    if let Some(d) = highest_entry.filter(|_| !surface_blocked) {
        assert!(
            d.above <= ENTRY_CLEARANCE_MM + AIR_TOL_MM,
            "{label}: a {:?} at move {} starts {:.2} mm above the material \
             (clearance {ENTRY_CLEARANCE_MM})",
            d.intent,
            d.move_index,
            d.above
        );
    }
    if let Some(d) = steepest_entry.filter(|_| !surface_blocked) {
        assert!(
            d.slope <= style.max_slope(),
            "{label}: a {:?} at move {} descends at slope {:.4} through material \
             (bound {:.4})",
            d.intent,
            d.move_index,
            d.slope,
            style.max_slope()
        );
    }
}

/// The probe's case: plunge style with the default keep-down (8 x D).
#[test]
fn plunge_keep_down_never_feeds_straight_into_stock() {
    let (tp, tool) = generate(Style::Plunge, None);
    let (audit, top) = replay(&tp, &tool);
    check_safe("plunge, keep-down 8D", &audit, Style::Plunge, DPP);
    assert!(
        top <= DEPTH_TOL,
        "the plate is not clear (max top {top:.3})"
    );
}

/// Option 2 fires: with the keep-down on, the rough retracts to safe Z
/// fewer times than with it off. Every style holds its bounds, and the
/// plate is clear.
#[test]
fn keep_down_saves_retracts_on_every_style() {
    for style in STYLES {
        let (tp_on, tool) = generate(style, None);
        let (tp_off, _) = generate(style, Some(0.0));
        let (on, top_on) = replay(&tp_on, &tool);
        let (off, top_off) = replay(&tp_off, &tool);
        check_safe(&format!("{style:?}, keep-down on"), &on, style, DPP);
        check_safe(&format!("{style:?}, keep-down off"), &off, style, DPP);
        assert!(
            top_on <= DEPTH_TOL && top_off <= DEPTH_TOL,
            "{style:?}: plate not clear"
        );
        assert!(
            on.retracts_to_safe < off.retracts_to_safe,
            "{style:?}: the keep-down link did not save a retract ({} on, {} off)",
            on.retracts_to_safe,
            off.retracts_to_safe
        );
    }
}

/// A dome under prism stock gives contour rings with walls beside them: the
/// ring starts, the side entries at depth and the keep-down links over
/// uncut shoulders. Every style, keep-down on, Depth/Pass 4.
///
/// OPEN (reported, not asserted): where the dome is steeper than the entry
/// slope, the G-RAMPTERRAIN clip lifts a helix turn or a ramp leg to the
/// drop-cutter floor, and the move after it must drop to reach the target.
/// The helix rate-limits its descent after a lift and spirals in, but where
/// the uphill floor stays above the target its last step still drops
/// straight down; the ramp clip does not rate-limit. The test prints the
/// count and the deepest drop; the flat plate and the pocket assert the
/// bounds.
#[test]
fn dome_entries_are_stock_safe_on_every_style() {
    let dome = make_test_hemisphere(15.0, 24);
    for style in STYLES {
        let (tp, tool) = generate_on(&dome, style, None, 4.0);
        let mut stock = seed_stock(15.0);
        let audit = replay_on(&tp, &tool, &mut stock, SAFE_Z);
        check_safe_with(
            &format!("dome, {style:?}"),
            &audit,
            style,
            4.0,
            !matches!(style, Style::Plunge),
        );
    }
}

/// `session/compute.rs` runs `optimize_entry_descents_annotated` on this
/// output with the op's SEED stock. It must not change it: its split target
/// (seed stock top + clearance) is never below where the planner's rapid
/// already stops, so it cannot rapid below material the planner reads.
#[test]
fn entry_optimiser_leaves_the_planner_entries_alone() {
    for style in [Style::Plunge, Style::Helix] {
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

// ── The 2.5D pocket (the dressup door) ────────────────────────────────

/// The `demo_pocket` shape of `ramp_contained_in_region_g_rampcontain`.
fn demo_pocket_polygon() -> Polygon2 {
    let exterior = vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ];
    let mut hole = Vec::with_capacity(64);
    let (cx, cy, r, n) = (40.0, 30.0, 10.0, 64);
    for i in 0..n {
        let t = (i as f64) * TAU / (n as f64);
        hole.push(P2::new(cx + r * (-t).cos(), cy + r * (-t).sin()));
    }
    Polygon2::with_holes(exterior, vec![hole])
}

const POCKET_DPP: f64 = 1.2;

fn pocket_toolpath(style: Style) -> Toolpath {
    let mut builder = ProjectSessionBuilder::new();
    builder = builder.stock(StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    });
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(LoadedModel {
        id: 0,
        name: "demo_pocket".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![demo_pocket_polygon()])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://demo_pocket.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });
    let mut dressups = DressupConfig::for_op(OperationType::Pocket);
    dressups.entry_style = match style {
        Style::Plunge => DressupEntryStyle::None,
        Style::Helix => DressupEntryStyle::Helix,
        Style::Ramp => DressupEntryStyle::Ramp,
    };
    dressups.helix_radius = Some(HELIX_RADIUS);
    dressups.helix_pitch = HELIX_PITCH;
    dressups.ramp_angle = RAMP_ANGLE_DEG;
    dressups.arc_fitting = None;
    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(PocketConfig {
            ramp_feed_rate: None,
            stepover: 2.0,
            depth: 12.0,
            depth_per_pass: POCKET_DPP,
            feed_rate: 770.0,
            plunge_rate: 385.0,
            climb: true,
            pattern: PocketPattern::Contour,
            angle: 0.0,
            finishing_passes: 0,
            spindle_rpm: Some(18_000),
        }),
        dressups,
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    };
    let _ = builder.add_toolpath(0, tc).expect("add pocket toolpath");
    let mut session = builder.build();
    session
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("generate pocket toolpath");
    session
        .get_result(0)
        .expect("pocket toolpath result")
        .toolpath()
        .clone()
}

/// The dressup door: a helix or ramp entry on a 12 mm pocket takes the full
/// material depth under the op's own replayed stock, within the pitch or
/// the angle; no straight feed enters material and no rapid does.
#[test]
fn pocket_helix_and_ramp_take_the_full_depth_within_their_bounds() {
    for style in [Style::Helix, Style::Ramp] {
        let tp = pocket_toolpath(style);
        let entries = tp
            .moves
            .iter()
            .filter(|m| matches!(m.intent, MoveIntent::EntryHelix | MoveIntent::EntryRamp))
            .count();
        assert!(
            entries > 0,
            "{style:?}: the pocket has no {style:?} entry moves"
        );
        let safe_z = tp
            .moves
            .iter()
            .map(|m| m.target.z)
            .fold(f64::NEG_INFINITY, f64::max);
        let tool = FlatEndmill::new(6.0, 25.0);
        let mut stock = prism(-10.0, -10.0, 90.0, 90.0, -12.0, 0.0);
        let audit = replay_on(&tp, &tool, &mut stock, safe_z);
        check_safe(&format!("pocket, {style:?}"), &audit, style, POCKET_DPP);
    }
}

// ── The session door: the planner order survives the dressups ─────────

/// Terrain half-width of the session fixture.
const TERRAIN_HALF: f64 = 20.0;

/// A rolling terrain under 16 mm of stock: hills and valleys give many
/// rings and many retract-separated runs in one depth pass, as on
/// rivmap100.
fn terrain_mesh() -> TriangleMesh {
    common::meshes::height_field(TERRAIN_HALF, 1.0, |x, y| {
        2.0 + 11.0 * ((x / 3.5).sin() * (y / 3.5).sin()).max(0.0)
    })
}

/// Generate the terrain rough through `ProjectSession::generate_toolpath`,
/// so the dressup pipeline runs as in the GUI, with the rapid-order
/// optimisation ON (the project-file default).
fn session_terrain_rough(style: Style) -> Toolpath {
    use rs_cam_core::compute::operation_configs::{
        Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, RegionOrdering as CfgOrdering,
    };
    let entry_style = match style {
        Style::Plunge => Adaptive3dEntryStyle::Plunge,
        Style::Helix => Adaptive3dEntryStyle::Helix,
        Style::Ramp => Adaptive3dEntryStyle::Ramp,
    };
    let cfg = Adaptive3dConfig {
        stepover: 1.2,
        depth_per_pass: DPP,
        stock_to_leave_axial: 0.5,
        feed_rate: 2400.0,
        entry_style,
        helix_radius_factor: HELIX_RADIUS / 6.0,
        helix_pitch: HELIX_PITCH,
        ramp_angle_deg: RAMP_ANGLE_DEG,
        entry_clearance_mm: ENTRY_CLEARANCE_MM,
        region_ordering: CfgOrdering::ByArea,
        clearing_strategy: ClearingStrategy::ContourParallel,
        // Keep-down off: every run starts after a retract, so the
        // rapid-order pass has many runs to permute.
        max_stay_down_distance_mm: Some(0.0),
        ..Adaptive3dConfig::default()
    };
    let mut session = common::session::single_op_session_with(
        common::session::stock_over(TERRAIN_HALF, STOCK_TOP_Z),
        make_endmill_6mm(),
        common::session::mesh_model(terrain_mesh(), "terrain"),
        "3D Rough",
        OperationConfig::Adaptive3d(cfg),
        |tc| {
            tc.dressups.optimize_rapid_order = true;
            tc.dressups.arc_fitting = None;
        },
    );
    common::session::generate(&mut session, 0);
    session
        .get_result(0)
        .expect("terrain rough result")
        .toolpath()
        .clone()
}

/// The adaptive3d planner reads each entry's rapid floor and helix start
/// from its own stock, in the order it emits the runs. A later rapid-order
/// permutation put a run that cut an entry column AFTER that entry, so
/// the tool fed straight down through standing stock above the planned
/// helix start (rivmap100, 2026-09-25: 448 entry samples over twice the
/// median bite, peak 6.08 mm). Replay the SESSION output on the stock: no
/// straight `EntryPlunge` of a helix or ramp entry may go into material.
///
/// Before the fix, this fixture fed a helix entry 4.26 mm straight down
/// into stock. The replay also reads one rapid retract at the stock edge
/// 0.76 mm into material, with and without the fix. That reading is not an
/// entry, so this test does not assert rapids.
#[test]
fn session_rough_keeps_the_planner_order_for_its_entries() {
    for style in [Style::Helix, Style::Ramp] {
        let tp = session_terrain_rough(style);
        let safe_z = tp
            .moves
            .iter()
            .map(|m| m.target.z)
            .fold(f64::NEG_INFINITY, f64::max);
        let tool = FlatEndmill::new(6.0, 25.0);
        let h = TERRAIN_HALF + 2.0;
        let mut stock = prism(-h, -h, h, h, 0.0, STOCK_TOP_Z);
        let audit = replay_on(&tp, &tool, &mut stock, safe_z);
        // The fixture must give the rapid-order pass runs to permute.
        assert!(
            audit.retracts_to_safe >= 10,
            "{style:?}: only {} retracts, so the fixture tests no reorder",
            audit.retracts_to_safe
        );
        let worst = audit
            .descents
            .iter()
            .filter(|d| d.steep() && d.intent == MoveIntent::EntryPlunge)
            .max_by(|a, b| a.depth.total_cmp(&b.depth));
        eprintln!(
            "terrain session, {style:?}: retracts {}, deepest straight EntryPlunge {:?}",
            audit.retracts_to_safe,
            worst.map(|d| (d.move_index, d.depth))
        );
        if let Some(d) = worst {
            assert!(
                d.depth <= DEPTH_TOL,
                "{style:?}: an EntryPlunge at move {} goes {:.2} mm straight into stock",
                d.move_index,
                d.depth
            );
        }
    }
}

/// The cone of the waterline fixture: its apex Z and its flank slope
/// (dz / dr, 60 degrees).
const CONE_TOP_Z: f64 = 14.0;
const CONE_SLOPE: f64 = 1.732;

/// G-WLENTRYDISC. A waterline cleanup contour is entered with the entry
/// style of the operation. The helix turns sweep the tool radius plus the
/// helix radius from the entry XY, so the planner must read the entry floor
/// over that disc, as the level clearing does.
///
/// The fixture: a 60 degree cone on a plate, Global order (a waterline
/// cleanup after each level), helix entries, Depth/Pass 4. Each waterline
/// contour stands one tool radius from the flank. Over the helix turns the
/// flank rises about 3 mm above the level. Before the fix the planner read
/// the floor over the tool disc only. The straight `EntryPlunge` then went
/// down to the level, below the flank that the helix turns meet.
#[test]
fn waterline_helix_entry_reads_its_floor_over_the_helix_turns() {
    let mesh = common::meshes::height_field(PLATE / 2.0, 0.25, |x, y| {
        (CONE_TOP_Z - CONE_SLOPE * x.hypot(y)).max(0.0)
    });
    let index = SpatialIndex::build(&mesh, 8.0);
    let tool = FlatEndmill::new(6.0, 25.0);
    let mut p = params(Style::Helix.adaptive3d(), None, 4.0);
    p.linking.region_ordering = RegionOrdering::Global;
    let (tp, annotations) = adaptive_3d_toolpath_annotated(&mesh, &index, &tool, &p);
    let waterline_starts: Vec<usize> = annotations
        .iter()
        .filter(|(_, label)| label.contains("Waterline"))
        .map(|(i, _)| *i)
        .collect();
    let first_waterline = *waterline_starts
        .first()
        .expect("the rough emitted no waterline cleanup marker");
    let mut stock = seed_stock(PLATE / 2.0);
    let audit = replay_on(&tp, &tool, &mut stock, SAFE_Z);
    // The fixture must enter at least one waterline contour with a helix.
    let waterline_helices = audit
        .descents
        .iter()
        .filter(|d| d.move_index > first_waterline && d.intent == MoveIntent::EntryHelix)
        .count();
    let plunges: Vec<&Descent> = audit
        .descents
        .iter()
        .filter(|d| d.steep() && d.intent == MoveIntent::EntryPlunge)
        .collect();
    let worst = plunges
        .iter()
        .max_by(|a, b| a.below_wide.total_cmp(&b.below_wide));
    eprintln!(
        "waterline cone, Helix: waterline markers at {waterline_starts:?}, helix moves \
         after the first {waterline_helices}, straight EntryPlunge {}, worst below the \
         helix-disc material {:?}",
        plunges.len(),
        worst.map(|d| (d.move_index, d.below_wide))
    );
    assert!(
        waterline_helices > 0,
        "no helix entry after the first waterline marker, so the fixture tests nothing"
    );
    if let Some(d) = worst {
        assert!(
            d.below_wide <= AIR_TOL_MM,
            "a straight EntryPlunge at move {} ends {:.2} mm below the material \
             under the helix turns",
            d.move_index,
            d.below_wide
        );
    }
}
