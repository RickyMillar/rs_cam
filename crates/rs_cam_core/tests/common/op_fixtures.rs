//! The all-op fixtures: one small fixture per `GeometryRequirement`, the
//! registry tool, the rest-arm prior op, and the post-precision fingerprint.
//!
//! Two suites generate every op in `OperationType::ALL` from these builders:
//! G-OPGOLDEN (`every_op_keeps_its_toolpath_g_opgolden.rs`, the blessed
//! fingerprints) and W4 (`every_op_conforms_w4.rs`, the conformance checks).
//! The builders and constants moved here from G-OPGOLDEN unchanged, so its
//! golden file needed no re-bless.
//!
//! # The fixtures
//!
//! - Stock, Items: a flat board below world Z0, a rectangle outline with
//!   two drill targets, and one alignment pin on the stock.
//! - Mesh, ItemsAndMesh: a spherical cap on a flat ground (flats, a slope,
//!   and a concave crease at the foot of the cap). `ItemsAndMesh` adds a
//!   curve model that the op projects onto the mesh.
//!
//! The tool comes from the registry row (`ToolConstraintsDef`): the first
//! required cutter kind, or a flat end mill when the row accepts any tool.
//! Every tool is `ToolConfig::new_default` for its kind.
//!
//! # The flipped frame
//!
//! [`Frame::Flipped`] puts the op on a `FaceUp::Bottom` setup and mirrors
//! every world input first: a drawing point `(x, y)` goes to `(x, -y)`, a
//! mesh vertex `(x, y, z)` goes to `(x, -y, H + 2·oz - z)`, and a
//! stock-local pin `(x, y)` goes to `(x, D - y)`. The stock is
//! centred on `y = 0`, so the Bottom transform takes each mirrored input
//! back onto the top-up input, moved by `-origin`. A frame-correct op then
//! emits the top-up toolpath moved by `-origin`.

#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::alignment_pins::AlignmentPin;
use rs_cam_core::compute::catalog::{GeometryRequirement, OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle, HeightMode, StockSource};
use rs_cam_core::compute::operation_configs::{AlignmentPinDrillConfig, FaceConfig, PocketConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::export::fingerprint::ToolpathFingerprint;
use rs_cam_core::gcode::post::{Decimals, grbl, grblhal, linuxcnc, mach3};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::ids::ModelId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::ops::face::FaceDirection;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{Item, ItemSet, Shape, SourceRef};
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSession, ProjectSessionBuilder, SetSimulationResolutionArgs,
    SimulationOptions, SimulationResolution, ToolpathConfig,
};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::produced::Produced;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use serde::{Deserialize, Serialize};

use super::fingerprint::fnv1a_debug;
use super::meshes::height_field;
use super::session::{mesh_model, pinned_heights, polygon_model, toolpath_config};

// ── The flat fixture (Stock, Polygons) ──────────────────────────────────

/// Flat board size (mm). The board hangs below world Z0: 2D ops cut at
/// negative Z.
pub const FLAT_X: f64 = 60.0;
pub const FLAT_Y: f64 = 40.0;
pub const FLAT_Z: f64 = 10.0;

/// Half extents (mm) of the outline the 2D ops cut. It leaves 10 mm of
/// board on X and 8 mm on Y, so an outside profile stays on the board.
pub const OUTLINE_HALF_X: f64 = 20.0;
pub const OUTLINE_HALF_Y: f64 = 12.0;

/// The two drill targets (mm), inside the outline.
pub const DRILL_TARGETS: [[f64; 2]; 2] = [[-10.0, 0.0], [10.0, 0.0]];

/// The one alignment pin (mm), on the board margin outside the outline:
/// world `(-25, 0)`. A pin is STOCK-LOCAL (`AlignmentPin`, `PinSetupMap`):
/// the pin drill adds the stock origin. Until W4 this read `[-25.0, 0.0]`,
/// which put the hole 55 mm off the board.
pub const PIN_XY: [f64; 2] = [-25.0 + FLAT_X / 2.0, FLAT_Y / 2.0];

// ── The mesh fixture (Mesh, Both) ───────────────────────────────────────

/// Half extent (mm) of the height field and of the stock.
pub const MESH_HALF: f64 = 15.0;
/// Height-field sample step (mm).
pub const MESH_STEP: f64 = 1.0;
/// Sphere radius (mm) and centre height (mm) of the cap. The cap height is
/// `SPHERE_R + SPHERE_CZ` and its foot radius is `sqrt(R² - CZ²)`.
pub const SPHERE_R: f64 = 10.0;
pub const SPHERE_CZ: f64 = -4.0;
/// Stock height (mm): 2 mm over the cap top.
pub const MESH_STOCK_Z: f64 = SPHERE_R + SPHERE_CZ + 2.0;

// ── The rest arm ────────────────────────────────────────────────────────

/// Simulation cell (mm) of the prior op in the rest arm.
pub const SIM_CELL_MM: f64 = 0.5;
/// Depth (mm) of the prior pocket on the flat fixture.
pub const PRIOR_POCKET_DEPTH: f64 = 3.0;
/// Stepover (mm) and depth per pass (mm) of the prior pocket: the values
/// `PocketConfig::default()` had when G-OPGOLDEN was blessed.
pub const PRIOR_POCKET_STEPOVER: f64 = 2.0;
pub const PRIOR_POCKET_DEPTH_PER_PASS: f64 = 1.5;

// ── Fixture builders ────────────────────────────────────────────────────

/// The registry's tool for `op`: the first required cutter kind, or a flat
/// end mill when the row accepts any tool.
pub fn tool_for(op: OperationType) -> ToolConfig {
    let kind = op
        .registry_entry()
        .tool_constraints
        .required_kinds
        .first()
        .map_or(ToolType::EndMill, |k| k.tool_type());
    ToolConfig::new_default(ToolId(0), kind)
}

/// True when the op runs on the mesh fixture.
pub fn on_mesh(op: OperationType) -> bool {
    matches!(
        op.spec().geometry,
        GeometryRequirement::Mesh | GeometryRequirement::ItemsAndMesh
    )
}

pub fn flat_stock() -> StockConfig {
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

pub fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(x0, y0),
        P2::new(x1, y0),
        P2::new(x1, y1),
        P2::new(x0, y1),
    ])
}

pub fn outline_model() -> LoadedModel {
    let mut model = polygon_model(
        vec![rectangle(
            -OUTLINE_HALF_X,
            -OUTLINE_HALF_Y,
            OUTLINE_HALF_X,
            OUTLINE_HALF_Y,
        )],
        "opgolden_outline",
    );
    // The drill targets are POINT items on layer `holes`, after the outline.
    let items: Vec<Item> = model
        .items
        .iter()
        .cloned()
        .chain(DRILL_TARGETS.iter().map(|&[x, y]| {
            Item::new(
                SourceRef::ToolpathModel,
                Some("holes".into()),
                Shape::Point(P2::new(x, y)),
            )
            .expect("a point is an item")
        }))
        .collect();
    model.items = Arc::new(ItemSet::new(items));
    model
}

pub fn mesh_stock() -> StockConfig {
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
pub fn cap_model() -> LoadedModel {
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

/// A flat plate at the stock top: a surface with nothing above it to cut.
pub fn plate_model() -> LoadedModel {
    mesh_model(
        height_field(MESH_HALF, MESH_STEP, |_, _| MESH_STOCK_Z),
        "w4_plate",
    )
}

/// A zigzag "river" over the cap, for Project Curve. `Polygon2::new`
/// makes it a CLOSED polygon of four points, so Project Curve also cuts
/// the closing segment from the last point back to the first. The golden
/// holds that toolpath; do not change the geometry here.
pub fn curve_model() -> LoadedModel {
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
pub fn flat_prior_model() -> LoadedModel {
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
/// mesh rest arm, and the G-OPGOLDEN diff then names `adaptive3d/fresh` too.
pub fn flat_prior_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: PRIOR_POCKET_STEPOVER,
        floor: rs_cam_core::compute::floor::Floor::top(PRIOR_POCKET_DEPTH),
        depth_per_pass: PRIOR_POCKET_DEPTH_PER_PASS,
        ..PocketConfig::default()
    })
}

// ── The flipped frame ───────────────────────────────────────────────────

/// The setup the case op sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    /// The identity setup: the op emits in world.
    TopUp,
    /// A `FaceUp::Bottom` setup over mirrored inputs. See the module doc.
    Flipped,
}

/// The mirror of a drawing point about `y = 0`.
fn mirror_xy(p: P2) -> P2 {
    P2::new(p.x, -p.y)
}

/// The mirrored copy of a model for [`Frame::Flipped`]. `stock` gives the
/// height and the Z origin of the mesh mirror.
fn mirrored_model(mut model: LoadedModel, stock: &StockConfig) -> LoadedModel {
    let mirror_poly = |poly: &Polygon2| {
        let mut m = Polygon2::with_holes(
            poly.exterior.iter().map(|p| mirror_xy(*p)).collect(),
            poly.holes
                .iter()
                .map(|h| h.iter().map(|p| mirror_xy(*p)).collect())
                .collect(),
        );
        m.closed = poly.closed;
        m
    };
    let mirrored: Vec<Item> = model
        .items
        .iter()
        .map(|item| {
            let shape = match item.shape() {
                Shape::Loop(p) => Shape::Loop(mirror_poly(p)),
                Shape::Path(p) => Shape::Path(mirror_poly(p)),
                Shape::Point(p) => Shape::Point(mirror_xy(*p)),
                Shape::Circle {
                    centre,
                    diameter,
                    ring,
                } => Shape::Circle {
                    centre: mirror_xy(*centre),
                    diameter: *diameter,
                    ring: mirror_poly(ring),
                },
                Shape::Face(_) => panic!("the flat fixtures hold no face"),
            };
            Item::new(item.source(), item.layer().map(Into::into), shape)
                .expect("a mirrored item keeps its invariants")
        })
        .collect();
    model.items = Arc::new(ItemSet::new(mirrored));
    if let Some(mesh) = &model.mesh {
        // Two reflections (Y and Z): the determinant is +1, so the
        // triangle winding keeps its sense.
        let z_sum = stock.z + 2.0 * stock.origin_z;
        let verts: Vec<P3> = mesh
            .vertices
            .iter()
            .map(|v| P3::new(v.x, -v.y, z_sum - v.z))
            .collect();
        model.mesh = Some(Arc::new(TriangleMesh::from_raw(
            verts,
            mesh.triangles.clone(),
        )));
    }
    model
}

// ── The session ─────────────────────────────────────────────────────────

/// What one case session holds before the op.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prior {
    /// The op alone.
    None,
    /// The rest-arm prior op first (a pocket on the flat fixture, a default
    /// Adaptive3d on the mesh fixture).
    RestPrior,
}

/// The geometry of one case session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fixture {
    /// The fixtures of the module doc.
    Standard,
    /// Nothing to cut: a 1 mm square outline (narrower than every registry
    /// tool), no drill target and no pin on the flat board; a flat plate at
    /// the stock top on the mesh fixture.
    NothingToCut,
}

/// Side (mm) of the [`Fixture::NothingToCut`] outline.
pub const TINY_OUTLINE_MM: f64 = 1.0;

/// The inputs of one case session.
pub struct CaseSpec<'a> {
    /// The toolpath name.
    pub name: &'a str,
    /// The op config. Its type picks the fixture and the tool.
    pub op: OperationConfig,
    pub prior: Prior,
    pub stock_source: StockSource,
    pub frame: Frame,
    pub fixture: Fixture,
    /// The tool of the case op. `None`: the registry tool ([`tool_for`]).
    pub tool: Option<ToolConfig>,
}

/// Build the session of one case. The case op is the last toolpath; its
/// index is returned. The prior op, when there is one, is toolpath 0.
pub fn case_session(spec: CaseSpec<'_>) -> (ProjectSession, usize) {
    let op_type = spec.op.op_type();
    let geometry = op_type.spec().geometry;
    let mesh = on_mesh(op_type);
    let empty = spec.fixture == Fixture::NothingToCut;
    let stock = match (mesh, empty) {
        (true, _) => mesh_stock(),
        (false, false) => flat_stock(),
        (false, true) => StockConfig {
            alignment_pins: Vec::new(),
            ..flat_stock()
        },
    };
    let flipped = spec.frame == Frame::Flipped;
    let stock = if flipped {
        StockConfig {
            alignment_pins: stock
                .alignment_pins
                .iter()
                // A pin is stock-local, so its mirror is about the stock's
                // own mid-line, `y = D / 2`.
                .map(|p| AlignmentPin::new(p.x, stock.y - p.y, p.diameter))
                .collect(),
            ..stock
        }
    } else {
        stock
    };
    let place = |model: LoadedModel| {
        if flipped {
            mirrored_model(model, &stock)
        } else {
            model
        }
    };
    let mut builder = ProjectSessionBuilder::new().stock(stock.clone());
    let setup = if flipped {
        builder.add_setup("Flipped".to_owned(), FaceUp::Bottom)
    } else {
        0
    };

    let tool_idx = builder.add_tool(spec.tool.unwrap_or_else(|| tool_for(op_type)));
    let tool_id = builder.tools()[tool_idx].id.0;

    let mut op = spec.op;
    let (model_id, heights) = if mesh {
        let surface = if empty { plate_model() } else { cap_model() };
        let cap_id = builder.add_model(place(surface));
        let model_id = if geometry == GeometryRequirement::ItemsAndMesh {
            if let OperationConfig::ProjectCurve(cfg) = &mut op {
                cfg.surface_model_id = Some(ModelId(cap_id));
            }
            builder.add_model(place(curve_model()))
        } else {
            cap_id
        };
        (model_id, Some(pinned_heights(MESH_STOCK_Z, 0.0)))
    } else if empty {
        let h = TINY_OUTLINE_MM / 2.0;
        let tiny = polygon_model(vec![rectangle(-h, -h, h, h)], "w4_tiny_outline");
        (builder.add_model(place(tiny)), None)
    } else {
        (builder.add_model(place(outline_model())), None)
    };

    if spec.prior == Prior::RestPrior {
        // The prior op: a flat end mill, which every fixture admits.
        let prior_tool_idx =
            builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
        let prior_tool_id = builder.tools()[prior_tool_idx].id.0;
        let prior: ToolpathConfig = if mesh {
            ToolpathConfig {
                shoe: Default::default(),
                heights: pinned_heights(MESH_STOCK_Z, 0.0),
                ..toolpath_config(
                    "prior",
                    OperationConfig::new_default(OperationType::Adaptive3d),
                    prior_tool_id,
                    builder.models()[0].id,
                )
            }
        } else {
            let prior_model = builder.add_model(place(flat_prior_model()));
            toolpath_config("prior", flat_prior_op(), prior_tool_id, prior_model)
        };
        let _ = builder
            .add_toolpath(setup, prior)
            .expect("add the prior op");
    }

    let mut config = toolpath_config(spec.name, op, tool_id, model_id);
    if let Some(h) = heights {
        config.heights = h;
    }
    config.stock_source = spec.stock_source;
    let index = builder
        .add_toolpath(setup, config)
        .expect("add the case op");
    (builder.build(), index)
}

/// Simulate every enabled toolpath of the session at [`SIM_CELL_MM`].
pub fn simulate(session: &mut ProjectSession) {
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
        .expect("the session simulates");
}

/// Generate the prior op (toolpath 0) and simulate it, so the case op can
/// read the remaining stock.
pub fn run_prior(session: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    let _ = session
        .generate_toolpath(0, &cancel)
        .expect("the prior op generates");
    simulate(session);
}

// ── The one door for one op's toolpath (D3) ────────────────────────────

/// Every dressup off: no entry, no link, no lead, no arc fit, no merge, no
/// feed optimisation and no rapid reorder. A test that dresses the op
/// output itself, or that reads the op output alone, starts from this.
pub fn no_dressups() -> DressupConfig {
    DressupConfig {
        entry_style: DressupEntryStyle::None,
        link_moves: None,
        lead_in_out: None,
        arc_fitting: None,
        segment_merge: None,
        dogbone: None,
        feed_optimization: false,
        optimize_rapid_order: false,
        ..DressupConfig::default()
    }
}

/// D3: the one door a test takes for the toolpath of one op. It builds a
/// one-op session, generates through `ProjectSession::generate_toolpath`
/// (the door the GUI, MCP and the CLI share) and returns the annotated
/// result: the op's runner output after the session stages (dressups,
/// boundary clip, entry split, obstacle step). `tweak` adjusts the
/// `ToolpathConfig` first; pass `|tc| tc.dressups = no_dressups()` for the
/// op output without dressups. D3 deleted the per-op public wrappers that
/// tests called before.
#[track_caller]
pub fn generated_op(
    stock: StockConfig,
    tool: ToolConfig,
    model: LoadedModel,
    op: OperationConfig,
    tweak: impl FnOnce(&mut ToolpathConfig),
) -> AnnotatedToolpath {
    let mut session = super::session::single_op_session_with(stock, tool, model, "op", op, tweak);
    super::session::generate(&mut session, 0);
    let annotated = session
        .get_result(0)
        .expect("the generated op has a result")
        .annotated();
    AnnotatedToolpath::clone(annotated)
}

/// The Face that the dressup sentries dress (`dressup_span_invariants`,
/// `arcfit_intent_key_cost_f1`, `transform_provenance_fingerprints`): a
/// 40 x 30 mm stock with its top at Z0, a 6 mm end mill, 4 mm stepover,
/// `OneWay` rows, `depth` deep in passes of 2 mm, feed 1500, plunge 500,
/// retract plane at Z 30. It comes through [`generated_op`] with
/// [`no_dressups`], so it is the Face runner's output after the session
/// stages. Before D3 these files called the deleted `face_toolpath` with
/// the same dials.
pub fn dressup_face(depth: f64) -> Toolpath {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    let stock = StockConfig {
        x: 40.0,
        y: 30.0,
        z: 5.0,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: -5.0,
        auto_from_model: false,
        alignment_pins: Vec::new(),
        ..StockConfig::default()
    };
    let op = OperationConfig::Face(FaceConfig {
        stepover: 4.0,
        depth,
        depth_per_pass: 2.0,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        stock_offset: 0.0,
        direction: FaceDirection::OneWay,
        ..FaceConfig::default()
    });
    let model = polygon_model(vec![rectangle(0.0, 0.0, 40.0, 30.0)], "dressup_face");
    generated_op(stock, tool, model, op, |tc| {
        tc.dressups = no_dressups();
        tc.heights.retract_z = HeightMode::Manual(30.0);
        tc.heights.clearance_z = HeightMode::Manual(30.0);
    })
    .toolpath
}

/// The count of moves with `MoveIntent::Unknown` (B11; the W6 census rule
/// that every emitted move carries a known intent).
pub fn unknown_intents(tp: &Toolpath) -> usize {
    tp.moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Unknown)
        .count()
}

// ── The fingerprint ─────────────────────────────────────────────────────

/// The finest decimals each shipped post emits.
pub fn finest_decimals() -> Decimals {
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

pub fn quantize(v: f64, decimals: usize) -> i64 {
    // The fixture coordinates and feeds are below 1e5, far inside the i64
    // range at any shipped precision, so the cast does not saturate.
    (v * scale(decimals)).round() as i64
}

pub fn round_to(v: f64, decimals: usize) -> f64 {
    quantize(v, decimals) as f64 / scale(decimals)
}

/// One move at the post precision: kind (0 rapid, 1 line, 2 CW arc, 3 CCW
/// arc), XYZ, arc IJ, feed and intent.
type QuantizedMove = (u8, [i64; 3], [i64; 2], i64, String);

/// One blessed record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Golden {
    /// `ok`, or the refusal text.
    pub outcome: String,
    pub move_count: usize,
    pub rapid_count: usize,
    pub linear_count: usize,
    pub arc_count: usize,
    /// A sum of many segments carries more noise than one coordinate, so
    /// the lengths are rounded one decade coarser than a coordinate.
    pub cutting_length_mm: f64,
    pub rapid_length_mm: f64,
    pub bbox_min: [f64; 3],
    pub bbox_max: [f64; 3],
    /// FNV-1a over the quantized moves, as hex (TOML holds only i64).
    pub moves_hash: String,
    /// The count of the spans (W6.3.0).
    pub span_count: usize,
    /// FNV-1a over `spans_valid` and `(kind, label, start, end)` of each
    /// span, in order, as hex. The payload is not in the hash (W6.3.0).
    pub spans_hash: String,
    /// W6.4.0: the digest of the rest output of a rest detector
    /// (`Produced::rest_grid`, `Produced::rest_regions`). `None` when no
    /// detector attached one, and then the record has no such field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub produced: Option<ProducedDigest>,
}

/// One polygon at the post precision: the exterior ring, then the holes.
type QuantizedPolygon = (Vec<(i64, i64)>, Vec<Vec<(i64, i64)>>);

/// The rest output of a rest detector, at the post precision (W6.4.0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProducedDigest {
    /// The cell count of the rest grid. 0 when there is no grid.
    pub rest_grid_cells: usize,
    /// FNV-1a over the grid spec, the quantized rest and surface Z of each
    /// cell (`NaN` is its own value) and the threshold, as hex.
    pub rest_grid_hash: String,
    /// The count of the rest region polygons.
    pub rest_region_count: usize,
    /// FNV-1a over the quantized vertices of each region polygon, as hex.
    pub rest_regions_hash: String,
}

impl ProducedDigest {
    /// The digest of `produced`, or `None` when it holds no rest output.
    pub fn of(produced: &Produced) -> Option<Self> {
        if produced.rest_grid.is_none() && produced.rest_regions.is_none() {
            return None;
        }
        let d = finest_decimals().xyz;
        let q32 = |v: f32| {
            if v.is_nan() {
                i64::MIN
            } else {
                quantize(f64::from(v), d)
            }
        };
        let (cells, grid_hash) = produced.rest_grid.as_deref().map_or_else(
            || (0, String::new()),
            |g| {
                let spec = (
                    g.grid.nx,
                    g.grid.ny,
                    quantize(g.grid.origin_x, d),
                    quantize(g.grid.origin_y, d),
                    quantize(g.grid.cell_mm, d),
                    quantize(g.threshold, d),
                );
                let rest: Vec<i64> = g.rest.iter().map(|&v| q32(v)).collect();
                let z: Vec<i64> = g.surface_z.iter().map(|&v| q32(v)).collect();
                (
                    g.rest.len(),
                    format!("{:016x}", fnv1a_debug(&(spec, rest, z))),
                )
            },
        );
        let (count, regions_hash) = produced.rest_regions.as_deref().map_or_else(
            || (0, String::new()),
            |regions| {
                let ring = |pts: &[P2]| -> Vec<(i64, i64)> {
                    pts.iter()
                        .map(|p| (quantize(p.x, d), quantize(p.y, d)))
                        .collect()
                };
                let quantized: Vec<QuantizedPolygon> = regions
                    .iter()
                    .map(|poly| {
                        (
                            ring(&poly.exterior),
                            poly.holes.iter().map(|h| ring(h)).collect(),
                        )
                    })
                    .collect();
                (regions.len(), format!("{:016x}", fnv1a_debug(&quantized)))
            },
        );
        Some(Self {
            rest_grid_cells: cells,
            rest_grid_hash: grid_hash,
            rest_region_count: count,
            rest_regions_hash: regions_hash,
        })
    }
}

impl Golden {
    /// The record of one generated result: its moves and its spans.
    pub fn of(annotated: &AnnotatedToolpath) -> Self {
        let tp = &annotated.toolpath;
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
            span_count: annotated.spans.len(),
            spans_hash: format!(
                "{:016x}",
                fnv1a_debug(&(annotated.spans_valid, span_digest(annotated)))
            ),
            produced: ProducedDigest::of(&annotated.produced),
        }
    }

    pub fn refused(message: &str) -> Self {
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
            span_count: 0,
            spans_hash: String::new(),
            produced: None,
        }
    }
}

/// One span as the golden hashes it: kind, label, start move, end move.
type SpanKey = (String, String, usize, usize);

/// The spans of a result as [`Golden::of`] hashes them, in order.
fn span_digest(annotated: &AnnotatedToolpath) -> Vec<SpanKey> {
    annotated
        .spans
        .iter()
        .map(|s| {
            (
                format!("{:?}", s.kind),
                s.label.to_string(),
                s.start_move,
                s.end_move,
            )
        })
        .collect()
}
