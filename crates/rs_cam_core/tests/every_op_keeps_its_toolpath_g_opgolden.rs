//! G-OPGOLDEN — every operation keeps its toolpath.
//!
//! # Why this exists
//!
//! The operation-architecture refactor (`planning/op_architecture_2026-10-14/`
//! PLAN.md, wave W0) moves every op adapter onto shared kernels. Each step
//! claims "no change in output". This file is the proof of that claim: it
//! generates every operation in `OperationType::ALL` through the real
//! session path and compares a fingerprint of each result with a blessed
//! file.
//!
//! # The path
//!
//! Each case builds a `ProjectSession` and calls `generate_toolpath`, the
//! door the GUI worker, MCP and the CLI share. So the fingerprint covers the
//! generator, the dressups, the boundary clip and the entry-descent split.
//!
//! # The fixtures
//!
//! One fixture for each `GeometryRequirement` kind:
//!
//! - Stock, Polygons: a flat board below world Z0, a rectangle outline with
//!   two drill targets, and one alignment pin on the stock.
//! - Mesh, Both: a spherical cap on a flat ground (flats, a slope, and a
//!   concave crease at the foot of the cap). `Both` adds a curve model that
//!   the op projects onto the mesh.
//!
//! The tool comes from the registry row (`ToolConstraintsDef`): the first
//! required cutter kind, or a flat end mill when the row accepts any tool.
//! Every tool is `ToolConfig::new_default` for its kind.
//!
//! # The arms
//!
//! - `fresh`: every op, default parameters, fresh stock.
//! - A non-default arm for each op with a cheap pattern or mode switch.
//! - `rest`: every op that reads the remaining stock in its adapter
//!   (`ctx.initial_stock`). A prior op cuts part of the stock, the session
//!   simulates it, and the op then generates with `FromRemainingStock`.
//!
//! # The fingerprint
//!
//! Move counts, cut and rapid length, the XYZ bounding box and an FNV-1a
//! hash (`common::fingerprint`) of the quantized moves. The quantum of a
//! coordinate is the finest precision a shipped post emits (the most
//! decimals over the four posts in `gcode::post`). A change below that
//! quantum cannot change a G-code word, and float noise across platforms
//! (about 1e-12 relative) is many decades below it. The hash also covers the
//! move kind, the feed (at the finest feed precision) and the move intent.
//!
//! # Blessing
//!
//! `RS_CAM_BLESS_OPGOLDEN=1` rewrites the golden file. Without it the test
//! compares and prints one line per changed field: op/arm, field, old, new.
//! Re-bless only the ops that a change names on purpose, and say so in the
//! commit body.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test every_op_keeps_its_toolpath_g_opgolden
//! RS_CAM_BLESS_OPGOLDEN=1 cargo test -p rs_cam_core -q --test every_op_keeps_its_toolpath_g_opgolden
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The per-field diff and the bless notice ARE the record.
#![allow(clippy::print_stderr)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::fingerprint::fnv1a_debug;
use common::meshes::height_field;
use common::session::{mesh_model, pinned_heights, polygon_model, toolpath_config};
use rayon::prelude::*;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::alignment_pins::AlignmentPin;
use rs_cam_core::compute::catalog::{GeometryRequirement, OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{
    AlignmentPinDrillConfig, DrillCycleType, PocketConfig, PocketPattern,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::export::fingerprint::ToolpathFingerprint;
use rs_cam_core::gcode::post::{Decimals, grbl, grblhal, linuxcnc, mach3};
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ModelId;
use rs_cam_core::io::dxf_input::{DrillTarget, DrillTargetKind};
use rs_cam_core::ops::profile::ProfileSide;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, ProjectSession, ProjectSessionBuilder, SetSimulationResolutionArgs, SimulationOptions,
    SimulationResolution, ToolpathConfig,
};
use rs_cam_core::toolpath::{MoveType, Toolpath};
use serde::{Deserialize, Serialize};

/// The environment switch that rewrites the golden file.
const BLESS_ENV: &str = "RS_CAM_BLESS_OPGOLDEN";

// ── The flat fixture (Stock, Polygons) ──────────────────────────────────

/// Flat board size (mm). The board hangs below world Z0: 2D ops cut at
/// negative Z.
const FLAT_X: f64 = 60.0;
const FLAT_Y: f64 = 40.0;
const FLAT_Z: f64 = 10.0;

/// Half extents (mm) of the outline the 2D ops cut. It leaves 10 mm of
/// board on X and 8 mm on Y, so an outside profile stays on the board.
const OUTLINE_HALF_X: f64 = 20.0;
const OUTLINE_HALF_Y: f64 = 12.0;

/// The two drill targets (mm), inside the outline.
const DRILL_TARGETS: [[f64; 2]; 2] = [[-10.0, 0.0], [10.0, 0.0]];

/// The one alignment pin (mm), on the board margin outside the outline.
const PIN_XY: [f64; 2] = [-25.0, 0.0];

// ── The mesh fixture (Mesh, Both) ───────────────────────────────────────

/// Half extent (mm) of the height field and of the stock.
const MESH_HALF: f64 = 15.0;
/// Height-field sample step (mm).
const MESH_STEP: f64 = 1.0;
/// Sphere radius (mm) and centre height (mm) of the cap. The cap height is
/// `SPHERE_R + SPHERE_CZ` and its foot radius is `sqrt(R² - CZ²)`.
const SPHERE_R: f64 = 10.0;
const SPHERE_CZ: f64 = -4.0;
/// Stock height (mm): 2 mm over the cap top.
const MESH_STOCK_Z: f64 = SPHERE_R + SPHERE_CZ + 2.0;

// ── The rest arm ────────────────────────────────────────────────────────

/// Simulation cell (mm) of the prior op in the rest arm.
const SIM_CELL_MM: f64 = 0.5;
/// Depth (mm) of the prior pocket on the flat fixture.
const PRIOR_POCKET_DEPTH: f64 = 3.0;
/// Stepover (mm) and depth per pass (mm) of the prior pocket: the values
/// `PocketConfig::default()` had when this file was blessed.
const PRIOR_POCKET_STEPOVER: f64 = 2.0;
const PRIOR_POCKET_DEPTH_PER_PASS: f64 = 1.5;

// ── Cases ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Fresh,
    Rest,
}

/// A change to an op's default parameters.
type Tweak = fn(&mut OperationConfig);

/// One generated toolpath: an op, an arm name, a parameter change and a
/// stock source.
#[derive(Clone, Copy)]
struct Case {
    op: OperationType,
    arm: &'static str,
    tweak: Tweak,
    source: Source,
}

impl Case {
    fn key(&self) -> String {
        format!("{}/{}", self.op.kind_str(), self.arm)
    }
}

fn no_tweak(_: &mut OperationConfig) {}

fn pocket_zigzag(op: &mut OperationConfig) {
    let OperationConfig::Pocket(cfg) = op else {
        unreachable!("pocket arm on a non-pocket op")
    };
    cfg.pattern = PocketPattern::Zigzag;
}

fn profile_inside(op: &mut OperationConfig) {
    let OperationConfig::Profile(cfg) = op else {
        unreachable!("profile arm on a non-profile op")
    };
    cfg.side = ProfileSide::Inside;
}

fn scallop_continuous(op: &mut OperationConfig) {
    let OperationConfig::Scallop(cfg) = op else {
        unreachable!("scallop arm on a non-scallop op")
    };
    cfg.continuous = !cfg.continuous;
}

fn drill_simple_cycle(op: &mut OperationConfig) {
    let OperationConfig::Drill(cfg) = op else {
        unreachable!("drill arm on a non-drill op")
    };
    cfg.cycle = DrillCycleType::Simple;
}

/// The ops whose adapter reads the remaining stock today
/// (`ctx.initial_stock` in `compute/execute/`): Pocket and 2D Adaptive
/// (`clearing_2d.rs`), Adaptive3d, Pencil and Unified Finish
/// (`finish_3d.rs`), the raster link stage of Drop Cutter, Scallop and
/// Waterline (`finish_raster.rs`), and Project Curve (`curve_engrave.rs`).
const REST_READERS: &[OperationType] = &[
    OperationType::Pocket,
    OperationType::Adaptive,
    OperationType::Adaptive3d,
    OperationType::Pencil,
    OperationType::UnifiedFinish,
    OperationType::DropCutter,
    OperationType::Scallop,
    OperationType::Waterline,
    OperationType::ProjectCurve,
];

fn cases() -> Vec<Case> {
    let mut cases: Vec<Case> = OperationType::ALL
        .iter()
        .map(|&op| Case {
            op,
            arm: "fresh",
            tweak: no_tweak,
            source: Source::Fresh,
        })
        .collect();
    let variants: [(OperationType, &'static str, Tweak); 4] = [
        (OperationType::Pocket, "zigzag", pocket_zigzag),
        (OperationType::Profile, "inside", profile_inside),
        (
            OperationType::Scallop,
            "continuous_flipped",
            scallop_continuous,
        ),
        (OperationType::Drill, "simple_cycle", drill_simple_cycle),
    ];
    for (op, arm, tweak) in variants {
        cases.push(Case {
            op,
            arm,
            tweak,
            source: Source::Fresh,
        });
    }
    for &op in REST_READERS {
        cases.push(Case {
            op,
            arm: "rest",
            tweak: no_tweak,
            source: Source::Rest,
        });
    }
    cases
}

// ── Fixture builders ────────────────────────────────────────────────────

/// The registry's tool for `op`: the first required cutter kind, or a flat
/// end mill when the row accepts any tool.
fn tool_for(op: OperationType) -> ToolConfig {
    let kind = op
        .registry_entry()
        .tool_constraints
        .required_kinds
        .first()
        .map_or(ToolType::EndMill, |k| k.tool_type());
    ToolConfig::new_default(ToolId(0), kind)
}

fn flat_stock() -> StockConfig {
    StockConfig {
        x: FLAT_X,
        y: FLAT_Y,
        z: FLAT_Z,
        origin_x: -FLAT_X / 2.0,
        origin_y: -FLAT_Y / 2.0,
        origin_z: -FLAT_Z,
        auto_from_model: false,
        alignment_pins: vec![AlignmentPin::new(
            PIN_XY[0],
            PIN_XY[1],
            AlignmentPinDrillConfig::default().diameter,
        )],
        ..StockConfig::default()
    }
}

fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(x0, y0),
        P2::new(x1, y0),
        P2::new(x1, y1),
        P2::new(x0, y1),
    ])
}

fn outline_model() -> rs_cam_core::session::LoadedModel {
    let mut model = polygon_model(
        vec![rectangle(
            -OUTLINE_HALF_X,
            -OUTLINE_HALF_Y,
            OUTLINE_HALF_X,
            OUTLINE_HALF_Y,
        )],
        "opgolden_outline",
    );
    model.drill_targets = Arc::new(
        DRILL_TARGETS
            .iter()
            .map(|&[x, y]| DrillTarget {
                x,
                y,
                layer: "holes".to_owned(),
                kind: DrillTargetKind::Point,
            })
            .collect(),
    );
    model
}

fn mesh_stock() -> StockConfig {
    StockConfig {
        x: 2.0 * MESH_HALF,
        y: 2.0 * MESH_HALF,
        z: MESH_STOCK_Z,
        origin_x: -MESH_HALF,
        origin_y: -MESH_HALF,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// A spherical cap on flat ground at `z = 0`.
fn cap_model() -> rs_cam_core::session::LoadedModel {
    let mesh = height_field(MESH_HALF, MESH_STEP, |x, y| {
        let r2 = x * x + y * y;
        let s = SPHERE_R * SPHERE_R - r2;
        if s <= 0.0 {
            0.0
        } else {
            (s.sqrt() + SPHERE_CZ).max(0.0)
        }
    });
    mesh_model(mesh, "opgolden_cap")
}

/// An open zigzag "river" over the cap, for Project Curve.
fn curve_model() -> rs_cam_core::session::LoadedModel {
    polygon_model(
        vec![Polygon2::new(vec![
            P2::new(-8.0, -4.0),
            P2::new(-3.0, 4.0),
            P2::new(3.0, -4.0),
            P2::new(8.0, 4.0),
        ])],
        "opgolden_curve",
    )
}

/// The flat-fixture prior op of the rest arm: a pocket over the left half
/// of the outline, so the rest op meets cut and uncut stock.
fn flat_prior_model() -> rs_cam_core::session::LoadedModel {
    polygon_model(
        vec![rectangle(
            -OUTLINE_HALF_X,
            -OUTLINE_HALF_Y,
            0.0,
            OUTLINE_HALF_Y,
        )],
        "opgolden_prior",
    )
}

/// The dials that shape the prior pocket are pinned here, so a change to
/// Pocket's defaults moves only the Pocket arms. The mesh prior is a
/// default Adaptive3d: a change to Adaptive3d's defaults also moves every
/// mesh rest arm, and the diff then names `adaptive3d/fresh` too.
fn flat_prior_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: PRIOR_POCKET_STEPOVER,
        depth: PRIOR_POCKET_DEPTH,
        depth_per_pass: PRIOR_POCKET_DEPTH_PER_PASS,
        ..PocketConfig::default()
    })
}

/// Build the session of one case. The case's op is the last toolpath.
fn session_for(case: &Case) -> (ProjectSession, usize) {
    let geometry = case.op.spec().geometry;
    let on_mesh = matches!(
        geometry,
        GeometryRequirement::Mesh | GeometryRequirement::Both
    );
    let stock = if on_mesh { mesh_stock() } else { flat_stock() };
    let mut builder = ProjectSessionBuilder::new().stock(stock);

    let tool_idx = builder.add_tool(tool_for(case.op));
    let tool_id = builder.tools()[tool_idx].id.0;

    let mut op = OperationConfig::new_default(case.op);
    (case.tweak)(&mut op);

    let (model_id, heights) = if on_mesh {
        let cap_id = builder.add_model(cap_model());
        let model_id = if geometry == GeometryRequirement::Both {
            if let OperationConfig::ProjectCurve(cfg) = &mut op {
                cfg.surface_model_id = Some(ModelId(cap_id));
            }
            builder.add_model(curve_model())
        } else {
            cap_id
        };
        (model_id, Some(pinned_heights(MESH_STOCK_Z, 0.0)))
    } else {
        (builder.add_model(outline_model()), None)
    };

    if let OperationConfig::AlignmentPinDrill(cfg) = &mut op {
        // The pin door snapshots the stock pins into the op; do the same.
        cfg.holes = vec![PIN_XY];
    }
    if let OperationConfig::Rest(cfg) = &mut op {
        // The Rest op models the corners a larger previous tool left. The
        // op has no default for that tool, so the fixture adds one of twice
        // the op tool's diameter.
        let prev = ToolConfig {
            diameter: 2.0 * builder.tools()[tool_idx].diameter,
            ..ToolConfig::new_default(ToolId(0), ToolType::EndMill)
        };
        let prev_idx = builder.add_tool(prev);
        cfg.prev_tool_id = Some(builder.tools()[prev_idx].id);
    }

    if case.source == Source::Rest {
        // The prior op: a flat end mill, which every fixture admits.
        let prior_tool_idx =
            builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
        let prior_tool_id = builder.tools()[prior_tool_idx].id.0;
        let prior: ToolpathConfig = if on_mesh {
            ToolpathConfig {
                heights: pinned_heights(MESH_STOCK_Z, 0.0),
                ..toolpath_config(
                    "prior",
                    OperationConfig::new_default(OperationType::Adaptive3d),
                    prior_tool_id,
                    model_id_of_cap(&builder),
                )
            }
        } else {
            let prior_model = builder.add_model(flat_prior_model());
            toolpath_config("prior", flat_prior_op(), prior_tool_id, prior_model)
        };
        let _ = builder.add_toolpath(0, prior).expect("add the prior op");
    }

    let mut config = toolpath_config(&case.key(), op, tool_id, model_id);
    if let Some(h) = heights {
        config.heights = h;
    }
    if case.source == Source::Rest {
        config.stock_source = StockSource::FromRemainingStock;
    }
    let index = builder.add_toolpath(0, config).expect("add the case op");
    (builder.build(), index)
}

/// The id of the cap model: the first model in a mesh session.
fn model_id_of_cap(builder: &ProjectSessionBuilder) -> usize {
    builder.models()[0].id
}

fn simulate(session: &mut ProjectSession) {
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(SIM_CELL_MM),
            },
        ))
        .expect("a positive cell");
    let cancel = AtomicBool::new(false);
    let _ = session
        .run_simulation(
            &SimulationOptions {
                resolution: SIM_CELL_MM,
                auto_resolution: false,
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("the prior op simulates");
}

/// Generate one case and fingerprint it. A refusal is a result too: a
/// refactor must not change it.
fn run(case: &Case) -> Golden {
    let (mut session, index) = session_for(case);
    let cancel = AtomicBool::new(false);
    if case.source == Source::Rest {
        let _ = session
            .generate_toolpath(0, &cancel)
            .expect("the prior op generates");
        simulate(&mut session);
    }
    match session.generate_toolpath(index, &cancel) {
        Ok(result) => Golden::of(result.toolpath()),
        Err(e) => Golden::refused(&e.to_string()),
    }
}

// ── The fingerprint ─────────────────────────────────────────────────────

/// The finest decimals each shipped post emits.
fn finest_decimals() -> Decimals {
    let posts = [grbl(), grblhal(), linuxcnc(), mach3()];
    Decimals {
        xyz: posts.iter().map(|p| p.decimals.xyz).max().unwrap(),
        feed: posts.iter().map(|p| p.decimals.feed).max().unwrap(),
        ijk: posts.iter().map(|p| p.decimals.ijk).max().unwrap(),
    }
}

fn scale(decimals: usize) -> f64 {
    10f64.powi(i32::try_from(decimals).unwrap())
}

fn quantize(v: f64, decimals: usize) -> i64 {
    // The fixture coordinates and feeds are below 1e5, far inside the i64
    // range at any shipped precision, so the cast does not saturate.
    (v * scale(decimals)).round() as i64
}

fn round_to(v: f64, decimals: usize) -> f64 {
    quantize(v, decimals) as f64 / scale(decimals)
}

/// One move at the post precision: kind (0 rapid, 1 line, 2 CW arc, 3 CCW
/// arc), XYZ, arc IJ, feed and intent.
type QuantizedMove = (u8, [i64; 3], [i64; 2], i64, String);

/// One blessed record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Golden {
    /// `ok`, or the refusal text.
    outcome: String,
    move_count: usize,
    rapid_count: usize,
    linear_count: usize,
    arc_count: usize,
    /// A sum of many segments carries more noise than one coordinate, so
    /// the lengths are rounded one decade coarser than a coordinate.
    cutting_length_mm: f64,
    rapid_length_mm: f64,
    bbox_min: [f64; 3],
    bbox_max: [f64; 3],
    /// FNV-1a over the quantized moves, as hex (TOML holds only i64).
    moves_hash: String,
}

impl Golden {
    fn of(tp: &Toolpath) -> Self {
        let d = finest_decimals();
        let fp = ToolpathFingerprint::from_toolpath(tp);
        let quantized: Vec<QuantizedMove> = tp
            .moves
            .iter()
            .map(|m| {
                let (kind, ij, feed) = match m.move_type {
                    MoveType::Rapid => (0_u8, [0, 0], 0.0),
                    MoveType::Linear { feed_rate } => (1, [0, 0], feed_rate),
                    MoveType::ArcCW { i, j, feed_rate } => {
                        (2, [quantize(i, d.ijk), quantize(j, d.ijk)], feed_rate)
                    }
                    MoveType::ArcCCW { i, j, feed_rate } => {
                        (3, [quantize(i, d.ijk), quantize(j, d.ijk)], feed_rate)
                    }
                };
                (
                    kind,
                    [
                        quantize(m.target.x, d.xyz),
                        quantize(m.target.y, d.xyz),
                        quantize(m.target.z, d.xyz),
                    ],
                    ij,
                    quantize(feed, d.feed),
                    format!("{:?}", m.intent),
                )
            })
            .collect();
        let length_decimals = d.xyz - 1;
        Self {
            outcome: "ok".to_owned(),
            move_count: fp.move_count,
            rapid_count: fp.rapid_count,
            linear_count: fp.linear_count,
            arc_count: fp.arc_cw_count + fp.arc_ccw_count,
            cutting_length_mm: round_to(fp.cutting_distance_mm, length_decimals),
            rapid_length_mm: round_to(fp.rapid_distance_mm, length_decimals),
            bbox_min: fp.bbox_min.map(|v| round_to(v, d.xyz)),
            bbox_max: fp.bbox_max.map(|v| round_to(v, d.xyz)),
            moves_hash: format!("{:016x}", fnv1a_debug(&quantized)),
        }
    }

    fn refused(message: &str) -> Self {
        Self {
            outcome: format!("refused: {message}"),
            move_count: 0,
            rapid_count: 0,
            linear_count: 0,
            arc_count: 0,
            cutting_length_mm: 0.0,
            rapid_length_mm: 0.0,
            bbox_min: [0.0; 3],
            bbox_max: [0.0; 3],
            moves_hash: String::new(),
        }
    }
}

// ── The golden file ─────────────────────────────────────────────────────

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/op_golden_g_opgolden.toml")
}

const GOLDEN_HEADER: &str = "\
# G-OPGOLDEN: blessed toolpath fingerprints of every operation.
# Written by tests/every_op_keeps_its_toolpath_g_opgolden.rs with
# RS_CAM_BLESS_OPGOLDEN=1. Do not edit by hand. Re-bless only the ops a
# change names on purpose, and say so in the commit body.

";

fn write_golden(actual: &BTreeMap<String, Golden>) {
    let body = toml::to_string(actual).expect("the goldens serialize");
    std::fs::write(golden_path(), format!("{GOLDEN_HEADER}{body}")).expect("write the goldens");
}

fn read_golden() -> BTreeMap<String, Golden> {
    let path = golden_path();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden file {} ({e}); bless it with {BLESS_ENV}=1",
            path.display()
        )
    });
    toml::from_str(&text).expect("the golden file parses")
}

/// One line per changed field: `op/arm  field  old -> new`.
fn diff(expected: &BTreeMap<String, Golden>, actual: &BTreeMap<String, Golden>) -> Vec<String> {
    let mut lines = Vec::new();
    let keys: BTreeSet<&String> = expected.keys().chain(actual.keys()).collect();
    for key in keys {
        match (expected.get(key), actual.get(key)) {
            (Some(_), None) => lines.push(format!("{key}: in the golden file, not generated")),
            (None, Some(_)) => lines.push(format!("{key}: generated, not in the golden file")),
            (Some(old), Some(new)) if old != new => {
                let old_v = toml::Value::try_from(old).expect("a table");
                let new_v = toml::Value::try_from(new).expect("a table");
                let (Some(old_t), Some(new_t)) = (old_v.as_table(), new_v.as_table()) else {
                    unreachable!("a struct serializes to a table")
                };
                for (field, ov) in old_t {
                    let nv = &new_t[field];
                    if ov != nv {
                        lines.push(format!("{key}  {field}  {ov} -> {nv}"));
                    }
                }
            }
            _ => {}
        }
    }
    lines
}

// ── The test ────────────────────────────────────────────────────────────

#[test]
fn every_op_keeps_its_toolpath() {
    let cases = cases();

    // The case list covers the whole registry.
    let covered: BTreeSet<&str> = cases
        .iter()
        .filter(|c| c.arm == "fresh")
        .map(|c| c.op.kind_str())
        .collect();
    let registered: BTreeSet<&str> = OperationType::ALL.iter().map(|op| op.kind_str()).collect();
    assert_eq!(covered, registered, "every registered op has a fresh arm");

    let actual: BTreeMap<String, Golden> = cases
        .par_iter()
        .map(|case| (case.key(), run(case)))
        .collect();

    // Non-vacuity: every op cuts something in its fresh arm, so a golden
    // of an empty toolpath cannot pass for a kept one.
    let empty: Vec<&String> = actual
        .iter()
        .filter(|(k, g)| k.ends_with("/fresh") && g.move_count == 0)
        .map(|(k, _)| k)
        .collect();
    assert!(
        empty.is_empty(),
        "these fresh arms emit no moves, so their golden proves nothing: {empty:?}\n{actual:#?}"
    );

    if std::env::var_os(BLESS_ENV).is_some() {
        write_golden(&actual);
        eprintln!(
            "blessed {} cases into {}",
            actual.len(),
            golden_path().display()
        );
        return;
    }

    let expected = read_golden();
    let lines = diff(&expected, &actual);
    assert!(
        lines.is_empty(),
        "the generated toolpaths moved ({} changes). Re-bless with {BLESS_ENV}=1 only for an \
         intended change:\n{}",
        lines.len(),
        lines.join("\n")
    );
}
