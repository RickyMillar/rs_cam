//! W6 — **no shipped generator emits a `MoveIntent::Retract` move that is
//! also a `MoveType::Linear` feed.**
//!
//! Oracle: `planning/perf_review_2026-08-19/DELTA_sim_w6_playback.md` §4
//! (the retract census), §4b and §4c.
//!
//! # Why this invariant is worth a sentry
//!
//! Two dexel grids disagree about what a Retract-tagged *feed* means, and
//! nothing today makes them agree:
//!
//! * The **metric** grid skips it. `dexel_stock/simulation.rs`'s
//!   `is_retract_feed` arm (`MoveIntent::Retract` **and**
//!   `MoveType::Linear { .. }`) routes the move through the Rapid-style
//!   accumulator: time is charged, `is_cutting = false`, and **no material
//!   is stamped**.
//! * The **playback** grid stamps it. `dexel_stock/playback.rs` /
//!   `simulation.rs::replay_moves` skip `MoveType::Rapid` and stamp every
//!   `Linear`/`Arc` regardless of intent.
//!
//! So one input class — a Retract-tagged Linear — would remove material in
//! `global_stock` / `group_stock` and remove none in the metric stock. That
//! is not a reporting difference: `group_stock` is what `prior_stocks`
//! snapshots, which is what `StockSource::FromRemainingStock` generation
//! reads, so it would be a difference in *generated G-code*.
//!
//! W6's answer to "how much playback time is retract?" was **0.0 %**,
//! because the divergence is currently unreachable: all 26 production
//! `MoveIntent::Retract` sites in `rs_cam_core` are `rapid_to_with_intent`,
//! and the playback loop has always skipped rapids. That census was
//! **static** — a source scan of every call site that can attach an intent
//! — and §4 says so explicitly: *"a generator added tomorrow could falsify
//! it, and there is no sentry pinning it."*
//!
//! This file is that sentry, and it is **dynamic**: it generates real
//! toolpaths through the production `ProjectSession::generate_toolpath`
//! entry point across the operation catalogue, at two dressup profiles, and
//! reads the emitted moves. A generator (or a dressup) that starts tagging
//! a *feed* as `Retract` fails here — loudly, with the family and the move
//! index — rather than silently activating a branch whose two
//! implementations do different things.
//!
//! # What a failure here means
//!
//! **Do not "fix" the generator to make this test pass.** A red result is
//! the finding: it means the metric/playback divergence just became
//! reachable from shipped generation, and W5B-F5 (which W6 deferred as
//! zero-value) is now load-bearing. The two grids must be reconciled
//! before the emitting generator ships.
//!
//! # Non-vacuity
//!
//! An invariant of the form "no move is X" passes trivially on an empty
//! population, which is exactly the failure mode `CLAUDE.md` names ("a gate
//! handed an empty population passes and looks healthy"). So the census
//! also asserts it *saw* the two halves of the predicate separately —
//! Retract-tagged moves (as rapids) and Linear feeds — that it covered a
//! minimum number of distinct operation types, and that the three
//! plunge-and-retract-loop families the retract tagging was introduced for
//! (`ProjectCurve`, `VCarve`, `Drill`) are individually present.
//!
//! ```text
//! cargo test -p rs_cam_core --release --test retract_intent_move_type_census_w6 -- --nocapture
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightMode, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, AdaptiveConfig, AlignmentPinDrillConfig, ChamferConfig, DrillConfig,
    DropCutterConfig, FaceConfig, HorizontalFinishConfig, InlayConfig, PencilConfig, PocketConfig,
    PocketPattern, ProfileConfig, ProjectCurveConfig, RadialFinishConfig, RampFinishConfig,
    RestConfig, ScallopConfig, SpiralFinishConfig, SteepShallowConfig, TraceConfig,
    UnifiedFinishConfig, VCarveConfig, WaterlineConfig, ZigzagConfig,
};
use rs_cam_core::compute::stock_config::{ModelId, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::profile::ProfileSide;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_core::toolpath::{MoveIntent, MoveType};

// ── Fixture geometry ────────────────────────────────────────────────────

/// Flat stock for the 2D / 2.5D arms. 2D ops cut at negative Z
/// (`project_2d_stock_z_frame`), so `origin_z` is negative and the stock
/// top lands on world z = 0.
const FLAT_X: f64 = 100.0;
const FLAT_Y: f64 = 80.0;
const FLAT_Z: f64 = 12.0;

/// Synthetic hemisphere for the 3D arms, with 2 mm of flat margin per side.
const HEMI_RADIUS_MM: f64 = 10.0;
const HEMI_STOCK_HALF_MM: f64 = HEMI_RADIUS_MM + 2.0;

/// Flat plate for the one arm that needs near-flat geometry.
const PLATE_MM: f64 = 24.0;
const PLATE_STOCK_Z_MM: f64 = 3.0;

// ── Dressup profiles ────────────────────────────────────────────────────

/// Which dressup stack a case is generated under.
///
/// A Retract-tagged Linear need not come from an operation generator: the
/// dressup layer rewrites and inserts moves too (lead-in/out arcs, link
/// bridges, retract strategy, arc fitting, feed optimisation). Generating
/// each family twice — registry default and everything the registry will
/// let us switch on — puts both layers in the census.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DressupProfile {
    /// Exactly what a fresh toolpath gets: `DressupConfig::for_op`.
    RegistryDefault,
    /// Every optional dressup flipped on, then `normalize_for_op` — i.e.
    /// the config a project file with all the dials up would load as, with
    /// the registry's own per-op strips still applied.
    Maximised,
}

impl DressupProfile {
    fn build(self, op: OperationType) -> DressupConfig {
        let mut cfg = DressupConfig::for_op(op);
        if self == Self::Maximised {
            cfg.lead_in_out = true;
            cfg.link_moves = true;
            cfg.arc_fitting = true;
            cfg.segment_merge = true;
            cfg.feed_optimization = true;
            cfg.optimize_rapid_order = true;
            // The registry, not this test, decides what is legal per op.
            cfg.normalize_for_op(op);
        }
        cfg
    }

    fn label(self) -> &'static str {
        match self {
            Self::RegistryDefault => "default",
            Self::Maximised => "maximised",
        }
    }
}

const PROFILES: [DressupProfile; 2] = [DressupProfile::RegistryDefault, DressupProfile::Maximised];

// ── One censused toolpath ───────────────────────────────────────────────

/// A move that violates the invariant: Retract-tagged AND a Linear feed.
#[derive(Debug, Clone)]
struct Offender {
    op: OperationType,
    profile: DressupProfile,
    move_index: usize,
    feed_rate: f64,
    target: [f64; 3],
}

#[derive(Debug, Clone)]
struct Row {
    op: OperationType,
    profile: DressupProfile,
    moves: usize,
    rapids: usize,
    linear_feeds: usize,
    arc_feeds: usize,
    retract_tagged: usize,
    /// Retract-tagged moves that are rapids — the shape every production
    /// site emits today.
    retract_rapids: usize,
    offenders: Vec<Offender>,
}

/// Generate one single-op session and read every emitted move.
///
/// Returns `Err` with the generator's own message when an operation cannot
/// be built on this fixture; an absent row and a clean row are different
/// statements and the caller reports them differently.
fn census(
    op_type: OperationType,
    profile: DressupProfile,
    mut session: ProjectSession,
) -> Result<Row, String> {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .map_err(|e| format!("generate: {e}"))?;
    let result = session
        .get_result(0)
        .ok_or_else(|| "generation reported success but stored no result".to_owned())?;
    let tp = result.toolpath();

    let mut row = Row {
        op: op_type,
        profile,
        moves: tp.moves.len(),
        rapids: 0,
        linear_feeds: 0,
        arc_feeds: 0,
        retract_tagged: 0,
        retract_rapids: 0,
        offenders: Vec::new(),
    };

    for (i, mv) in tp.moves.iter().enumerate() {
        match mv.move_type {
            MoveType::Rapid => row.rapids += 1,
            MoveType::Linear { .. } => row.linear_feeds += 1,
            MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => row.arc_feeds += 1,
        }
        if mv.intent == MoveIntent::Retract {
            row.retract_tagged += 1;
            if matches!(mv.move_type, MoveType::Rapid) {
                row.retract_rapids += 1;
            }
            // THE INVARIANT. Both halves of the metric path's
            // `is_retract_feed` predicate, verbatim.
            if let MoveType::Linear { feed_rate } = mv.move_type {
                row.offenders.push(Offender {
                    op: op_type,
                    profile,
                    move_index: i,
                    feed_rate,
                    target: [mv.target.x, mv.target.y, mv.target.z],
                });
            }
        }
    }

    Ok(row)
}

// ── Session builders ────────────────────────────────────────────────────

fn toolpath_config(
    op: OperationConfig,
    profile: DressupProfile,
    tool_id: usize,
    model_id: usize,
    heights: HeightsConfig,
) -> ToolpathConfig {
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(0),
        name: format!("{op_type:?}"),
        enabled: true,
        operation: op,
        dressups: profile.build(op_type),
        heights,
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
        rest_analysis: RestAnalysisConfig::default(),
    }
}

fn add_tool(session: &mut ProjectSession, kind: ToolType, diameter: f64, name: &str) -> usize {
    let mut tool = ToolConfig::new_default(ToolId(0), kind);
    tool.diameter = diameter;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = name.to_owned();
    let idx = session.add_tool(tool);
    session.tools()[idx].id.0
}

fn flat_stock(session: &mut ProjectSession) {
    session.set_stock_config(StockConfig {
        x: FLAT_X,
        y: FLAT_Y,
        z: FLAT_Z,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -FLAT_Z,
        auto_from_model: false,
        ..StockConfig::default()
    });
}

fn add_rect_polygon(session: &mut ProjectSession) -> usize {
    let poly = Polygon2::new(vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ]);
    session.add_model(LoadedModel {
        id: 0,
        name: "w6_census_rect".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![poly])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://w6_census_rect.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    })
}

/// A flat-stock session carrying exactly one 2D / 2.5D operation.
fn flat_session(
    op: OperationConfig,
    profile: DressupProfile,
    tool_kind: ToolType,
    tool_diameter: f64,
) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    flat_stock(&mut session);
    let tool_id = add_tool(&mut session, tool_kind, tool_diameter, "w6 census tool");
    let model_id = add_rect_polygon(&mut session);
    session
        .add_toolpath(
            0,
            toolpath_config(op, profile, tool_id, model_id, HeightsConfig::default()),
        )
        .expect("add toolpath");
    session
}

/// The Rest arm needs a *previous* tool to have left something behind, so
/// it is the one flat case that cannot use [`flat_session`].
fn rest_session(profile: DressupProfile) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    flat_stock(&mut session);
    let prev_tool_id = add_tool(&mut session, ToolType::EndMill, 12.0, "w6 census prev");
    let tool_id = add_tool(&mut session, ToolType::EndMill, 3.0, "w6 census rest");
    let model_id = add_rect_polygon(&mut session);
    let op = OperationConfig::Rest(RestConfig {
        prev_tool_id: Some(ToolId(prev_tool_id)),
        stepover: 1.0,
        depth: 4.0,
        depth_per_pass: 2.0,
        feed_rate: 800.0,
        plunge_rate: 400.0,
        angle: 0.0,
        spindle_rpm: Some(18_000),
    });
    session
        .add_toolpath(
            0,
            toolpath_config(op, profile, tool_id, model_id, HeightsConfig::default()),
        )
        .expect("add toolpath");
    session
}

fn hemi_stock(session: &mut ProjectSession) {
    session.set_stock_config(StockConfig {
        x: 2.0 * HEMI_STOCK_HALF_MM,
        y: 2.0 * HEMI_STOCK_HALF_MM,
        z: HEMI_RADIUS_MM,
        origin_x: -HEMI_STOCK_HALF_MM,
        origin_y: -HEMI_STOCK_HALF_MM,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
}

fn add_hemisphere(session: &mut ProjectSession, id: usize) -> usize {
    session.add_model(LoadedModel {
        id,
        name: "w6_census_hemisphere".to_owned(),
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_hemisphere(
            HEMI_RADIUS_MM,
            8,
        ))),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://w6_census_hemisphere.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    })
}

/// A hemisphere session carrying exactly one surface operation.
///
/// Surface ops carry no depth dial, so the Z band is pinned by hand — left
/// on `Auto` a waterline band collapses to zero height and the op emits
/// nothing, which would put a vacuous row in the census.
fn hemi_session(
    op: OperationConfig,
    profile: DressupProfile,
    tool_kind: ToolType,
    tool_diameter: f64,
) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    hemi_stock(&mut session);
    let tool_id = add_tool(&mut session, tool_kind, tool_diameter, "w6 census ball");
    let model_id = add_hemisphere(&mut session, 0);
    let heights = HeightsConfig {
        top_z: HeightMode::Manual(HEMI_RADIUS_MM),
        bottom_z: HeightMode::Manual(0.0),
        ..HeightsConfig::default()
    };
    session
        .add_toolpath(0, toolpath_config(op, profile, tool_id, model_id, heights))
        .expect("add toolpath");
    session
}

/// A flat-plate session — the only fixture `HorizontalFinish` can be
/// measured on (`CLAUDE.md`: "useless on terrain"; on the hemisphere it
/// emits nothing at all).
fn plate_session(op: OperationConfig, profile: DressupProfile) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(StockConfig {
        x: PLATE_MM,
        y: PLATE_MM,
        z: PLATE_STOCK_Z_MM,
        origin_x: -PLATE_MM / 2.0,
        origin_y: -PLATE_MM / 2.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let tool_id = add_tool(&mut session, ToolType::BallNose, 6.0, "w6 census ball");
    let model_id = session.add_model(LoadedModel {
        id: 0,
        name: "w6_census_plate".to_owned(),
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_flat(PLATE_MM))),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://w6_census_plate.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });
    let heights = HeightsConfig {
        top_z: HeightMode::Manual(PLATE_STOCK_Z_MM),
        bottom_z: HeightMode::Manual(0.0),
        ..HeightsConfig::default()
    };
    session
        .add_toolpath(0, toolpath_config(op, profile, tool_id, model_id, heights))
        .expect("add toolpath");
    session
}

/// The `ProjectCurve` arm — a 2D "river" ring projected onto the dome. One
/// of the three plunge-and-retract-loop families the retract tagging was
/// introduced for (`CLAUDE.md`, Step 1 2026-05-19).
fn project_curve_session(profile: DressupProfile) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    hemi_stock(&mut session);
    let tool_id = add_tool(&mut session, ToolType::BallNose, 3.0, "w6 census ball 3mm");
    let surface_model_id = add_hemisphere(&mut session, 0);
    let curve = Polygon2::new(vec![
        P2::new(-6.0, -3.0),
        P2::new(-2.0, 3.0),
        P2::new(2.0, -3.0),
        P2::new(6.0, 3.0),
    ]);
    let curve_model_id = session.add_model(LoadedModel {
        id: 1,
        name: "w6_census_river".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![curve])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://w6_census_river.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });
    let op = OperationConfig::ProjectCurve(ProjectCurveConfig {
        depth: 1.0,
        point_spacing: 0.4,
        feed_rate: 800.0,
        plunge_rate: 400.0,
        surface_model_id: Some(ModelId(surface_model_id)),
        spindle_rpm: Some(18_000),
        ..ProjectCurveConfig::default()
    });
    session
        .add_toolpath(
            0,
            toolpath_config(
                op,
                profile,
                tool_id,
                curve_model_id,
                HeightsConfig::default(),
            ),
        )
        .expect("add toolpath");
    session
}

// ── The censused population ─────────────────────────────────────────────

type FlatCase = (fn() -> OperationConfig, ToolType, f64);

/// Flat-stock cases: 2.5D clearing, 2D contour, and both drill cycles.
fn flat_cases() -> Vec<FlatCase> {
    vec![
        // ── 2.5D clearing / rough ──
        (
            || {
                OperationConfig::Pocket(PocketConfig {
                    stepover: 3.0,
                    depth: 6.0,
                    depth_per_pass: 3.0,
                    feed_rate: 1000.0,
                    plunge_rate: 400.0,
                    climb: true,
                    pattern: PocketPattern::Contour,
                    angle: 0.0,
                    finishing_passes: 0,
                    spindle_rpm: Some(18_000),
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        (
            || {
                OperationConfig::Zigzag(ZigzagConfig {
                    stepover: 3.0,
                    depth: 3.0,
                    depth_per_pass: 3.0,
                    feed_rate: 1000.0,
                    plunge_rate: 400.0,
                    angle: 45.0,
                    spindle_rpm: Some(18_000),
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        (
            || {
                OperationConfig::Face(FaceConfig {
                    stepover: 3.0,
                    depth: 2.0,
                    depth_per_pass: 2.0,
                    feed_rate: 1500.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..FaceConfig::default()
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        (
            || {
                OperationConfig::Adaptive(AdaptiveConfig {
                    stepover: 2.0,
                    depth: 6.0,
                    depth_per_pass: 3.0,
                    feed_rate: 1500.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..AdaptiveConfig::default()
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        // ── 2D contour ──
        (
            || {
                OperationConfig::Profile(ProfileConfig {
                    side: ProfileSide::Outside,
                    depth: 6.0,
                    depth_per_pass: 3.0,
                    feed_rate: 1000.0,
                    plunge_rate: 400.0,
                    climb: true,
                    tab_count: 0,
                    tab_width: 6.0,
                    tab_height: 2.0,
                    finishing_passes: 0,
                    compensation:
                        rs_cam_core::compute::operation_configs::CompensationType::InComputer,
                    spindle_rpm: Some(18_000),
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        (
            || {
                OperationConfig::Trace(TraceConfig {
                    depth: 3.0,
                    depth_per_pass: 1.5,
                    feed_rate: 800.0,
                    plunge_rate: 400.0,
                    spindle_rpm: Some(18_000),
                    ..TraceConfig::default()
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        (
            || {
                OperationConfig::VCarve(VCarveConfig {
                    max_depth: 4.0,
                    stepover: 0.5,
                    feed_rate: 800.0,
                    plunge_rate: 400.0,
                    spindle_rpm: Some(18_000),
                    ..VCarveConfig::default()
                })
            },
            ToolType::VBit,
            12.7,
        ),
        (
            || {
                OperationConfig::Chamfer(ChamferConfig {
                    chamfer_width: 1.5,
                    tip_offset: 0.1,
                    feed_rate: 800.0,
                    plunge_rate: 400.0,
                    spindle_rpm: Some(18_000),
                })
            },
            ToolType::VBit,
            12.7,
        ),
        (
            || {
                OperationConfig::Inlay(InlayConfig {
                    pocket_depth: 3.0,
                    feed_rate: 800.0,
                    plunge_rate: 400.0,
                    spindle_rpm: Some(18_000),
                    ..InlayConfig::default()
                })
            },
            ToolType::VBit,
            12.7,
        ),
        // ── Drill cycles: Z-only kinematics, and the family whose
        // retracts `CLAUDE.md` cites by name. ──
        (
            || {
                OperationConfig::Drill(DrillConfig {
                    depth: 8.0,
                    selected_holes: Some(vec![
                        [20.0, 20.0],
                        [40.0, 20.0],
                        [60.0, 20.0],
                        [40.0, 40.0],
                    ]),
                    ..DrillConfig::default()
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        (
            || {
                OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
                    holes: vec![[15.0, 15.0], [65.0, 45.0]],
                    ..AlignmentPinDrillConfig::default()
                })
            },
            ToolType::EndMill,
            6.0,
        ),
    ]
}

type HemiCase = (fn() -> OperationConfig, ToolType, f64);

/// Hemisphere cases: the 3D finish family plus the 3D rougher.
fn hemi_cases() -> Vec<HemiCase> {
    vec![
        (
            || {
                OperationConfig::Adaptive3d(Adaptive3dConfig {
                    stepover: 2.0,
                    depth_per_pass: 2.5,
                    feed_rate: 1500.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..Adaptive3dConfig::default()
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        (
            || {
                OperationConfig::DropCutter(DropCutterConfig {
                    stepover: 1.5,
                    feed_rate: 1000.0,
                    plunge_rate: 400.0,
                    min_z: 0.0,
                    slope_from: 0.0,
                    slope_to: 90.0,
                    spindle_rpm: Some(18_000),
                    scallop_height: None,
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            || {
                OperationConfig::Waterline(WaterlineConfig {
                    z_step: 1.5,
                    sampling: 0.4,
                    feed_rate: 1000.0,
                    plunge_rate: 400.0,
                    continuous: false,
                    spindle_rpm: Some(18_000),
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            || {
                OperationConfig::Scallop(ScallopConfig {
                    scallop_height: 0.1,
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..ScallopConfig::default()
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            || {
                OperationConfig::UnifiedFinish(UnifiedFinishConfig {
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..UnifiedFinishConfig::default()
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            || {
                OperationConfig::SteepShallow(SteepShallowConfig {
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..SteepShallowConfig::default()
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            || {
                OperationConfig::RampFinish(RampFinishConfig {
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..RampFinishConfig::default()
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            || {
                OperationConfig::SpiralFinish(SpiralFinishConfig {
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..SpiralFinishConfig::default()
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            || {
                OperationConfig::RadialFinish(RadialFinishConfig {
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..RadialFinishConfig::default()
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            || {
                OperationConfig::Pencil(PencilConfig {
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..PencilConfig::default()
                })
            },
            ToolType::BallNose,
            3.0,
        ),
    ]
}

fn collect_rows() -> (Vec<Row>, Vec<String>) {
    let mut rows = Vec::new();
    let mut skipped = Vec::new();

    let push = |label: String,
                outcome: Result<Row, String>,
                rows: &mut Vec<Row>,
                skipped: &mut Vec<String>| match outcome {
        Ok(r) => rows.push(r),
        Err(e) => skipped.push(format!("{label}: {e}")),
    };

    for profile in PROFILES {
        for (build, kind, dia) in flat_cases() {
            let op = build();
            let op_type = op.op_type();
            let label = format!("{op_type:?} [{}]", profile.label());
            let outcome = census(op_type, profile, flat_session(op, profile, kind, dia));
            push(label, outcome, &mut rows, &mut skipped);
        }

        let label = format!("Rest [{}]", profile.label());
        let outcome = census(OperationType::Rest, profile, rest_session(profile));
        push(label, outcome, &mut rows, &mut skipped);

        for (build, kind, dia) in hemi_cases() {
            let op = build();
            let op_type = op.op_type();
            let label = format!("{op_type:?} [{}]", profile.label());
            let outcome = census(op_type, profile, hemi_session(op, profile, kind, dia));
            push(label, outcome, &mut rows, &mut skipped);
        }

        let label = format!("ProjectCurve [{}]", profile.label());
        let outcome = census(
            OperationType::ProjectCurve,
            profile,
            project_curve_session(profile),
        );
        push(label, outcome, &mut rows, &mut skipped);

        let horizontal = OperationConfig::HorizontalFinish(HorizontalFinishConfig {
            stepover: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            spindle_rpm: Some(18_000),
            ..HorizontalFinishConfig::default()
        });
        let label = format!("HorizontalFinish [{}]", profile.label());
        let outcome = census(
            OperationType::HorizontalFinish,
            profile,
            plate_session(horizontal, profile),
        );
        push(label, outcome, &mut rows, &mut skipped);
    }

    (rows, skipped)
}

// ── The tests ───────────────────────────────────────────────────────────

/// **The sentry.** No shipped generator, under either dressup profile,
/// emits a `MoveIntent::Retract` move that is also a `MoveType::Linear`
/// feed.
///
/// See the module doc for why a failure here must NOT be fixed by changing
/// the generator.
#[test]
fn no_shipped_generator_emits_a_retract_tagged_linear_feed() {
    let (rows, skipped) = collect_rows();

    for s in &skipped {
        println!("W6 CENSUS SKIPPED | {s}");
    }
    for r in &rows {
        println!(
            "W6 CENSUS | {:<18} | {:<9} | moves {:>7} | rapid {:>7} | linear {:>7} | \
             arc {:>6} | retract {:>5} (rapid {:>5}) | offenders {}",
            format!("{:?}", r.op),
            r.profile.label(),
            r.moves,
            r.rapids,
            r.linear_feeds,
            r.arc_feeds,
            r.retract_tagged,
            r.retract_rapids,
            r.offenders.len(),
        );
    }

    // ── Non-vacuity, BEFORE the invariant. An empty census satisfies
    // "no move is X" trivially; these four assertions are what make the
    // green result a statement about generated motion.
    let total_moves: usize = rows.iter().map(|r| r.moves).sum();
    let total_linear: usize = rows.iter().map(|r| r.linear_feeds).sum();
    let total_retract: usize = rows.iter().map(|r| r.retract_tagged).sum();
    // `OperationType` is not `Ord`, so the coverage sets are keyed on its
    // `Debug` name — which is what the failure messages print anyway.
    //
    // Coverage counts only op types that actually EMITTED motion. A family
    // that generates successfully but returns an empty toolpath is not
    // evidence about a move predicate, and letting it count would let this
    // sentry hollow out one family at a time while the number held. (One
    // is expected today: `Pencil` finds no concave valleys on a convex
    // dome, so its row is legitimately empty on this fixture.)
    let ops_covered: BTreeSet<String> = rows
        .iter()
        .filter(|r| r.moves > 0)
        .map(|r| format!("{:?}", r.op))
        .collect();
    let ops_with_retracts: BTreeSet<String> = rows
        .iter()
        .filter(|r| r.retract_tagged > 0)
        .map(|r| format!("{:?}", r.op))
        .collect();
    let empty_rows: BTreeSet<String> = rows
        .iter()
        .filter(|r| r.moves == 0)
        .map(|r| format!("{:?}", r.op))
        .collect();

    println!(
        "W6 CENSUS TOTALS | rows {} | ops with motion {} | moves {} | linear feeds {} | \
         retract-tagged {} | ops emitting retracts {} | empty rows {empty_rows:?}",
        rows.len(),
        ops_covered.len(),
        total_moves,
        total_linear,
        total_retract,
        ops_with_retracts.len(),
    );

    assert!(
        ops_covered.len() >= 20,
        "census covered only {} operation types that emitted motion — too thin \
         to pin the invariant. Empty rows: {empty_rows:?}. Skipped: {skipped:#?}",
        ops_covered.len()
    );
    assert!(
        total_linear > 0,
        "census saw ZERO Linear feeds; the invariant's second half was \
         never exercised"
    );
    assert!(
        total_retract > 0,
        "census saw ZERO `MoveIntent::Retract` moves; the invariant's first \
         half was never exercised. Either intent tagging regressed or the \
         fixtures stopped generating."
    );
    assert!(
        ops_with_retracts.len() >= 5,
        "only {} operation types emitted any retract-tagged move ({:?}) — one \
         family carrying the whole non-vacuity claim is not a census",
        ops_with_retracts.len(),
        ops_with_retracts
    );

    // The three plunge-and-retract-loop families named in `CLAUDE.md`'s
    // Step 1 (2026-05-19) note must each be present by name. Coverage
    // counted in aggregate can drop exactly the families this invariant is
    // most about and still clear a threshold.
    for required in [
        OperationType::ProjectCurve,
        OperationType::VCarve,
        OperationType::Drill,
    ] {
        let seen = rows.iter().any(|r| r.op == required && r.moves > 0);
        assert!(
            seen,
            "{required:?} produced no censused moves — it is one of the three \
             plunge-and-retract-loop families this invariant exists for. \
             Skipped: {skipped:#?}"
        );
    }

    // ── THE INVARIANT ──
    let offenders: Vec<&Offender> = rows.iter().flat_map(|r| r.offenders.iter()).collect();
    assert!(
        offenders.is_empty(),
        "{} move(s) are BOTH `MoveIntent::Retract` AND `MoveType::Linear` — \
         the metric grid skips these and the playback grid stamps them, so \
         this input class makes the two dexel stocks disagree, and \
         `group_stock` feeds `StockSource::FromRemainingStock` generation. \
         DO NOT silence this by retagging the generator; reconcile the two \
         kernels (W5B-F5) first. See DELTA_sim_w6_playback.md §4b.\n{}",
        offenders.len(),
        offenders
            .iter()
            .take(20)
            .map(|o| format!(
                "  {:?} [{}] move #{} feed {:.1} mm/min at ({:.3}, {:.3}, {:.3})",
                o.op,
                o.profile.label(),
                o.move_index,
                o.feed_rate,
                o.target[0],
                o.target[1],
                o.target[2],
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // Every retract that WAS emitted is a rapid — the positive form of the
    // same statement, and the one §4 actually measured (26/26 sites are
    // `rapid_to_with_intent`). Stated separately so a future arc-tagged
    // retract is reported as its own finding rather than sliding past the
    // Linear-only assertion above.
    let mut non_rapid: BTreeMap<String, usize> = BTreeMap::new();
    for r in &rows {
        let arcs = r.retract_tagged - r.retract_rapids - r.offenders.len();
        if arcs > 0 {
            *non_rapid.entry(format!("{:?}", r.op)).or_insert(0) += arcs;
        }
    }
    assert!(
        non_rapid.is_empty(),
        "retract-tagged ARC moves emitted by {non_rapid:?} — the playback \
         kernel stamps arcs too, so this is the same divergence in a \
         different move type"
    );
}

/// The other half of §4's static argument, pinned as a unit fact: nothing
/// can turn an existing `Rapid` into a `Linear` while carrying its intent
/// along.
///
/// `MoveType::with_feed_rate` is the ONLY `move_type` rewrite in the
/// workspace (`feed_modulation.rs`, `feedopt.rs`). If it ever mapped
/// `Rapid` to a feed, every one of the 26 `rapid_to_with_intent(Retract)`
/// sites would start producing the offending input class *after*
/// generation — which the census above, reading the generator's output,
/// would still see, but only for ops whose dressups run modulation.
#[test]
fn with_feed_rate_never_promotes_a_rapid_into_a_feed() {
    assert!(
        matches!(MoveType::Rapid.with_feed_rate(1234.0), MoveType::Rapid),
        "`with_feed_rate` re-typed a Rapid; a Retract-tagged rapid can now \
         become a Retract-tagged Linear feed post-generation"
    );
    assert!(
        matches!(
            MoveType::Linear { feed_rate: 100.0 }.with_feed_rate(1234.0),
            MoveType::Linear { feed_rate } if (feed_rate - 1234.0).abs() < 1e-9
        ),
        "`with_feed_rate` no longer sets the feed on a Linear move"
    );
}
