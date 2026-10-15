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
//! `common::op_fixtures`: one fixture for each `GeometryRequirement` kind
//! (a flat board with an outline, two drill targets and a pin; a spherical
//! cap on flat ground, plus a curve for `Both`), and the registry tool. W4
//! (`every_op_conforms_w4.rs`) builds its sessions from the same module.
//!
//! # The arms
//!
//! - `fresh`: every op, default parameters, fresh stock.
//! - A non-default arm for each op with a cheap pattern or mode switch.
//! - `rest`: every op that reads the remaining stock in its adapter
//!   (`Prepared::territory`). A prior op cuts part of the stock, the session
//!   simulates it, and the op then generates with `FromRemainingStock`.
//! - `regions` (W6.3.0): each height-field op whose territory is a region
//!   set, with a `ModelOutline` boundary rectangle that cuts through the
//!   cap ([`REGION_RECT`]). It pins the region keep of each kernel and the
//!   ring boundaries of Scallop.
//! - W6.3.0 non-default arms of the height-field ops: an active slope band
//!   and the link stage of Drop Cutter, the iso field of Scallop, the
//!   outside-in Spiral and the bottom-up Ramp.
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
//! The record also holds a span digest (W6.3.0): the span count and a hash
//! of each span's kind, label and move range.
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
use std::sync::atomic::AtomicBool;

use common::op_fixtures::{
    CaseSpec, Fixture, Frame, Golden, Prior, case_session, run_prior, unknown_intents,
};
use rayon::prelude::*;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType, RestUse};
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, StockSource,
};
use rs_cam_core::compute::operation_configs::{
    Adaptive3dEntryStyle, ClearingStrategy, DrillCycleType, PocketPattern, ProjectCurveDirection,
    ProjectCurveProjection, ProjectCurveSide, RegionOrdering, SpiralDirection,
};
use rs_cam_core::finish::pencil::PencilDetector;
use rs_cam_core::finish::unified_finish::ClaimsReference;
use rs_cam_core::ops::profile::ProfileSide;
use rs_cam_core::session::{
    AddModelArgs, Command, ProjectSession, SetBoundaryConfigArgs, SetToolpathModelArgs,
};

/// The environment switch that rewrites the golden file.
const BLESS_ENV: &str = "RS_CAM_BLESS_OPGOLDEN";

// ── Cases ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Fresh,
    Rest,
}

/// A change to an op's default parameters.
type Tweak = fn(&mut OperationConfig);

/// One generated toolpath: an op, an arm name, a parameter change, a
/// stock source and the boundary.
#[derive(Clone, Copy)]
struct Case {
    op: OperationType,
    arm: &'static str,
    tweak: Tweak,
    source: Source,
    /// True: a `ModelOutline` boundary of [`REGION_RECT`] (W6.3.0).
    regions: bool,
    /// The setup of the op (W6.5.0: the flipped Project Curve arm).
    frame: Frame,
    /// The curve the op projects (W6.5.0).
    curve: CurveModel,
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

/// An active slope band: only the cap's mid slopes.
fn drop_cutter_slope_band(op: &mut OperationConfig) {
    let OperationConfig::DropCutter(cfg) = op else {
        unreachable!("drop cutter arm on another op")
    };
    cfg.slope_from = 20.0;
    cfg.slope_to = 60.0;
}

/// The link stage on (`hookup_mm > 0`).
fn drop_cutter_hookup(op: &mut OperationConfig) {
    let OperationConfig::DropCutter(cfg) = op else {
        unreachable!("drop cutter arm on another op")
    };
    cfg.hookup_mm = DROP_CUTTER_HOOKUP_MM;
}

/// The slope band splits each row at the flat cap top, and the link
/// stage joins the two runs over it.
fn drop_cutter_slope_band_hookup(op: &mut OperationConfig) {
    drop_cutter_slope_band(op);
    drop_cutter_hookup(op);
}

fn scallop_iso_field(op: &mut OperationConfig) {
    let OperationConfig::Scallop(cfg) = op else {
        unreachable!("scallop arm on a non-scallop op")
    };
    cfg.iso_field = true;
}

/// W6.4.0: the claims pipeline on (the crease detector and node).
fn unified_claims(op: &mut OperationConfig) {
    let OperationConfig::UnifiedFinish(cfg) = op else {
        unreachable!("unified arm on another op")
    };
    cfg.pencil_claims = true;
}

/// W6.4.0: the claims pipeline against the machined stock, with the S4
/// territory clip.
fn unified_claims_rest_clip(op: &mut OperationConfig) {
    unified_claims(op);
    let OperationConfig::UnifiedFinish(cfg) = op else {
        unreachable!("unified arm on another op")
    };
    cfg.claims_reference = ClaimsReference::MachinedStock;
    cfg.territory_clip = true;
}

/// W6.4.0: no C2 monotone cells and no intra-region link stage.
fn unified_no_c2_no_hookup(op: &mut OperationConfig) {
    let OperationConfig::UnifiedFinish(cfg) = op else {
        unreachable!("unified arm on another op")
    };
    cfg.monotone_cell_decomposition = false;
    cfg.intra_region_hookup_mm = 0.0;
}

fn pencil_rest_depth(op: &mut OperationConfig) {
    let OperationConfig::Pencil(cfg) = op else {
        unreachable!("pencil arm on another op")
    };
    cfg.detector = PencilDetector::RestDepth;
}

fn pencil_curvature(op: &mut OperationConfig) {
    let OperationConfig::Pencil(cfg) = op else {
        unreachable!("pencil arm on another op")
    };
    cfg.detector = PencilDetector::Curvature;
}

/// W6.4.6: the link stage of Steep and Shallow on.
fn steep_shallow_hookup(op: &mut OperationConfig) {
    let OperationConfig::SteepShallow(cfg) = op else {
        unreachable!("steep and shallow arm on another op")
    };
    cfg.hookup_mm = STEEP_SHALLOW_HOOKUP_MM;
}

fn spiral_outside_in(op: &mut OperationConfig) {
    let OperationConfig::SpiralFinish(cfg) = op else {
        unreachable!("spiral arm on another op")
    };
    cfg.direction = SpiralDirection::OutsideIn;
}

fn ramp_bottom_up(op: &mut OperationConfig) {
    let OperationConfig::RampFinish(cfg) = op else {
        unreachable!("ramp arm on another op")
    };
    cfg.order_bottom_up = true;
}

/// W6.5.0: By Area order, a helix entry and the ContourSpiral strategy.
/// It covers the three config-to-planner enum maps of 3D Rough, the By
/// Area detection and the planner engagement samples.
fn adaptive3d_by_area_helix_spiral(op: &mut OperationConfig) {
    let OperationConfig::Adaptive3d(cfg) = op else {
        unreachable!("adaptive3d arm on another op")
    };
    cfg.region_ordering = RegionOrdering::ByArea;
    cfg.entry_style = Adaptive3dEntryStyle::Helix;
    cfg.clearing_strategy = ClearingStrategy::ContourSpiral;
}

/// W6.5.0: a ramp entry and the AgentSearch strategy (the other arms of
/// the two enum maps).
fn adaptive3d_ramp_agent(op: &mut OperationConfig) {
    let OperationConfig::Adaptive3d(cfg) = op else {
        unreachable!("adaptive3d arm on another op")
    };
    cfg.entry_style = Adaptive3dEntryStyle::Ramp;
    cfg.clearing_strategy = ClearingStrategy::AgentSearch;
}

/// W6.5.0: the Adaptive strategy (the last arm of the strategy map).
fn adaptive3d_adaptive(op: &mut OperationConfig) {
    let OperationConfig::Adaptive3d(cfg) = op else {
        unreachable!("adaptive3d arm on another op")
    };
    cfg.clearing_strategy = ClearingStrategy::Adaptive;
}

/// W6.5.0: radius compensation inside the closed fixture curve.
fn project_curve_side_inside(op: &mut OperationConfig) {
    let OperationConfig::ProjectCurve(cfg) = op else {
        unreachable!("project curve arm on another op")
    };
    cfg.side = ProjectCurveSide::Inside;
}

/// W6.5.0: the point projection.
fn project_curve_point(op: &mut OperationConfig) {
    let OperationConfig::ProjectCurve(cfg) = op else {
        unreachable!("project curve arm on another op")
    };
    cfg.projection = ProjectCurveProjection::Point;
}

/// W6.5.0: the link stage of Project Curve on (`chain_distance_mm`).
fn project_curve_chained(op: &mut OperationConfig) {
    let OperationConfig::ProjectCurve(cfg) = op else {
        unreachable!("project curve arm on another op")
    };
    cfg.chain_distance_mm = PROJECT_CURVE_CHAIN_MM;
}

/// W6.5.0: projection from below.
fn project_curve_from_below(op: &mut OperationConfig) {
    let OperationConfig::ProjectCurve(cfg) = op else {
        unreachable!("project curve arm on another op")
    };
    cfg.direction = ProjectCurveDirection::FromBelow;
}

/// The Project Curve link-stage cap (mm) of the `split_chained` arm. It is
/// wider than each gap of [`split_curve_model`] (1 mm and 2 mm).
const PROJECT_CURVE_CHAIN_MM: f64 = 3.0;

/// The curve a Project Curve case projects.
#[derive(Clone, Copy)]
enum CurveModel {
    /// The fixture curve (`common::op_fixtures::curve_model`).
    Fixture,
    /// [`split_curve_model`]: three open curves (the link stage).
    Split,
    /// [`square_curve_model`]: one closed square (the side compensation).
    Square,
}

/// The W6.5.0 arms: op, arm, tweak, frame, curve. All on fresh stock.
const W650_CASES: &[(OperationType, &str, Tweak, Frame, CurveModel)] = &[
    (
        OperationType::Adaptive3d,
        "by_area_helix_spiral",
        adaptive3d_by_area_helix_spiral,
        Frame::TopUp,
        CurveModel::Fixture,
    ),
    (
        OperationType::Adaptive3d,
        "ramp_agent",
        adaptive3d_ramp_agent,
        Frame::TopUp,
        CurveModel::Fixture,
    ),
    (
        OperationType::Adaptive3d,
        "adaptive",
        adaptive3d_adaptive,
        Frame::TopUp,
        CurveModel::Fixture,
    ),
    (
        OperationType::ProjectCurve,
        "square",
        no_tweak,
        Frame::TopUp,
        CurveModel::Square,
    ),
    (
        OperationType::ProjectCurve,
        "side_inside",
        project_curve_side_inside,
        Frame::TopUp,
        CurveModel::Square,
    ),
    (
        OperationType::ProjectCurve,
        "point",
        project_curve_point,
        Frame::TopUp,
        CurveModel::Fixture,
    ),
    (
        OperationType::ProjectCurve,
        "split",
        no_tweak,
        Frame::TopUp,
        CurveModel::Split,
    ),
    (
        OperationType::ProjectCurve,
        "split_chained",
        project_curve_chained,
        Frame::TopUp,
        CurveModel::Split,
    ),
    (
        OperationType::ProjectCurve,
        "from_below_topup",
        project_curve_from_below,
        Frame::TopUp,
        CurveModel::Fixture,
    ),
    (
        OperationType::ProjectCurve,
        "from_below_flipped",
        project_curve_from_below,
        Frame::Flipped,
        CurveModel::Fixture,
    ),
];

/// Three open curves over the cap, with gaps of 1 mm and 2 mm between
/// them. Each curve is one chain, so the link stage has two gaps to join.
fn split_curve_model() -> rs_cam_core::session::LoadedModel {
    use rs_cam_core::geo::P2;
    use rs_cam_core::polygon::Polygon2;
    common::session::polygon_model(
        vec![
            Polygon2::open_path(vec![P2::new(-7.0, -2.0), P2::new(-2.0, 2.0)]),
            Polygon2::open_path(vec![P2::new(-1.0, 2.0), P2::new(3.0, -2.0)]),
            Polygon2::open_path(vec![P2::new(5.0, -2.0), P2::new(7.0, 2.0)]),
        ],
        "opgolden_split_curve",
    )
}

/// One closed 8 mm square over the cap.
fn square_curve_model() -> rs_cam_core::session::LoadedModel {
    common::session::polygon_model(
        vec![common::op_fixtures::rectangle(-4.0, -4.0, 4.0, 4.0)],
        "opgolden_square_curve",
    )
}

/// Project `model` with toolpath `index`.
fn use_curve(session: &mut ProjectSession, index: usize, model: rs_cam_core::session::LoadedModel) {
    let model_id = session
        .apply(Command::AddModel(AddModelArgs {
            model: Box::new(model),
        }))
        .expect("add the curve model")
        .created
        .expect("the add reports the model id");
    let _ = session
        .apply(Command::SetToolpathModel(SetToolpathModelArgs {
            index,
            model_id,
        }))
        .expect("bind the curve model");
}

/// The Drop Cutter link-stage cap (mm) of the `hookup` arms. It is wider
/// than the flat cap top that the slope band leaves out (slope below 20
/// degrees: radius `R sin 20` = 3.4 mm), so the stage can join a row over it.
const DROP_CUTTER_HOOKUP_MM: f64 = 10.0;

/// The Steep and Shallow link-stage cap (mm) of the `rest_hookup` arm: the
/// Drop Cutter cap, so the two raster families link on one rule.
const STEEP_SHALLOW_HOOKUP_MM: f64 = DROP_CUTTER_HOOKUP_MM;

/// The boundary rectangle `[x0, y0, x1, y1]` (mm) of the `regions` arms.
/// It is off centre and cuts through the cap (foot radius about 9.2 mm),
/// so a kernel keeps only a part of the cap.
const REGION_RECT: [f64; 4] = [-12.0, -10.0, 5.0, 9.0];

/// The height-field ops whose territory is a region set (W6.3.0). Each
/// gets a `regions` arm.
const REGION_OPS: &[OperationType] = &[
    OperationType::DropCutter,
    OperationType::Scallop,
    OperationType::HorizontalFinish,
    OperationType::RadialFinish,
    OperationType::SpiralFinish,
    OperationType::RampFinish,
];

/// The ops whose adapter reads the remaining stock today
/// (`Prepared::territory` in `compute/execute/`): Pocket, Zigzag, Profile,
/// Trace, Face and 2D Adaptive (`clearing_2d.rs`, the Layered rest rules
/// of W3.4), Chamfer (`curve_engrave.rs`, the Layered level skip of W3.5),
/// Drill (`drilling.rs`, the stock column of D8), Adaptive3d,
/// Pencil and Unified Finish (`finish_3d.rs`), the raster link stage of
/// Drop Cutter and Scallop, the contour filter of Waterline
/// (`finish_raster.rs`, W6.1), Project Curve (`curve_engrave.rs`), and the
/// contour filter of both halves of Steep and Shallow (`finish_3d.rs`,
/// W6.4.6). Each is an op whose `Capabilities::rest_use` is not
/// `RestUse::None`.
const REST_READERS: &[OperationType] = &[
    OperationType::Pocket,
    OperationType::Zigzag,
    OperationType::Profile,
    OperationType::Trace,
    OperationType::Face,
    OperationType::Chamfer,
    OperationType::Drill,
    OperationType::Adaptive,
    OperationType::Adaptive3d,
    OperationType::Pencil,
    OperationType::UnifiedFinish,
    OperationType::DropCutter,
    OperationType::Scallop,
    OperationType::Waterline,
    OperationType::ProjectCurve,
    OperationType::SteepShallow,
];

fn cases() -> Vec<Case> {
    let mut cases: Vec<Case> = OperationType::ALL
        .iter()
        .map(|&op| Case {
            op,
            arm: "fresh",
            tweak: no_tweak,
            source: Source::Fresh,
            regions: false,
            frame: Frame::TopUp,
            curve: CurveModel::Fixture,
        })
        .collect();
    let variants: [(OperationType, &'static str, Tweak, Source, bool); 19] = [
        (
            OperationType::Pocket,
            "zigzag",
            pocket_zigzag,
            Source::Fresh,
            false,
        ),
        (
            OperationType::Profile,
            "inside",
            profile_inside,
            Source::Fresh,
            false,
        ),
        (
            OperationType::Scallop,
            "continuous_flipped",
            scallop_continuous,
            Source::Fresh,
            false,
        ),
        (
            OperationType::Drill,
            "simple_cycle",
            drill_simple_cycle,
            Source::Fresh,
            false,
        ),
        // W6.3.0: the non-default arms of the height-field ops.
        (
            OperationType::DropCutter,
            "slope_band",
            drop_cutter_slope_band,
            Source::Fresh,
            false,
        ),
        (
            OperationType::DropCutter,
            "slope_band_hookup",
            drop_cutter_slope_band_hookup,
            Source::Fresh,
            false,
        ),
        (
            OperationType::DropCutter,
            "regions_hookup_rest",
            drop_cutter_hookup,
            Source::Rest,
            true,
        ),
        (
            OperationType::Scallop,
            "iso_field",
            scallop_iso_field,
            Source::Fresh,
            false,
        ),
        (
            OperationType::Scallop,
            "regions_rest",
            no_tweak,
            Source::Rest,
            true,
        ),
        (
            OperationType::SpiralFinish,
            "outside_in",
            spiral_outside_in,
            Source::Fresh,
            false,
        ),
        (
            OperationType::RampFinish,
            "bottom_up",
            ramp_bottom_up,
            Source::Fresh,
            false,
        ),
        // W6.4.0: the composed-surface ops and the rest detector stage.
        (
            OperationType::UnifiedFinish,
            "claims",
            unified_claims,
            Source::Fresh,
            false,
        ),
        (
            OperationType::UnifiedFinish,
            "claims_rest_clip",
            unified_claims_rest_clip,
            Source::Rest,
            false,
        ),
        (
            OperationType::UnifiedFinish,
            "no_c2_no_hookup",
            unified_no_c2_no_hookup,
            Source::Fresh,
            false,
        ),
        (
            OperationType::Pencil,
            "rest_depth",
            pencil_rest_depth,
            Source::Fresh,
            false,
        ),
        (
            OperationType::Pencil,
            "rest_depth_rest",
            pencil_rest_depth,
            Source::Rest,
            false,
        ),
        (
            OperationType::Pencil,
            "curvature",
            pencil_curvature,
            Source::Fresh,
            false,
        ),
        // W6.4.6: Steep and Shallow over the remaining stock in a region
        // (its `rest` arm is a `REST_READERS` arm), and its link stage over
        // the remaining stock.
        (
            OperationType::SteepShallow,
            "rest_regions",
            no_tweak,
            Source::Rest,
            true,
        ),
        (
            OperationType::SteepShallow,
            "rest_hookup",
            steep_shallow_hookup,
            Source::Rest,
            false,
        ),
    ];
    for (op, arm, tweak, source, regions) in variants {
        cases.push(Case {
            op,
            arm,
            tweak,
            source,
            regions,
            frame: Frame::TopUp,
            curve: CurveModel::Fixture,
        });
    }
    for (op, arm, tweak, frame, curve) in W650_CASES {
        cases.push(Case {
            op: *op,
            arm,
            tweak: *tweak,
            source: Source::Fresh,
            regions: false,
            frame: *frame,
            curve: *curve,
        });
    }
    for &op in REGION_OPS {
        cases.push(Case {
            op,
            arm: "regions",
            tweak: no_tweak,
            source: Source::Fresh,
            regions: true,
            frame: Frame::TopUp,
            curve: CurveModel::Fixture,
        });
    }
    for &op in REST_READERS {
        cases.push(Case {
            op,
            arm: "rest",
            tweak: no_tweak,
            source: Source::Rest,
            regions: false,
            frame: Frame::TopUp,
            curve: CurveModel::Fixture,
        });
    }
    cases
}

/// Put the `regions` boundary on toolpath `index`: a polygon model of
/// [`REGION_RECT`] and a `ModelOutline` boundary on it, `Center`
/// containment. A `ModelOutline` boundary fills the region set of the op
/// (`pre_boundary_regions`).
fn add_region_boundary(session: &mut ProjectSession, index: usize) {
    let [x0, y0, x1, y1] = REGION_RECT;
    let model = common::session::polygon_model(
        vec![common::op_fixtures::rectangle(x0, y0, x1, y1)],
        "opgolden_regions",
    );
    let model_id = session
        .apply(Command::AddModel(AddModelArgs {
            model: Box::new(model),
        }))
        .expect("add the region model")
        .created
        .expect("the add reports the model id");
    let _ = session
        .apply(Command::SetBoundaryConfig(SetBoundaryConfigArgs {
            index,
            boundary: BoundaryConfig {
                enabled: true,
                source: BoundarySource::ModelOutline {
                    model_id,
                    holes: false,
                },
                containment: BoundaryContainment::Center,
                offset: 0.0,
            },
        }))
        .expect("set the region boundary");
}

/// One generated case: its fingerprint and the count of its moves with
/// `MoveIntent::Unknown` (B11).
struct Generated {
    golden: Golden,
    unknown_intents: usize,
}

/// Generate one case and fingerprint it. A refusal is a result too: a
/// refactor must not change it.
fn run(case: &Case) -> Generated {
    let mut op = OperationConfig::new_default(case.op);
    (case.tweak)(&mut op);
    let rest = case.source == Source::Rest;
    let (mut session, index) = case_session(CaseSpec {
        name: &case.key(),
        op,
        prior: if rest { Prior::RestPrior } else { Prior::None },
        stock_source: if rest {
            StockSource::FromRemainingStock
        } else {
            StockSource::Fresh
        },
        frame: case.frame,
        fixture: Fixture::Standard,
        tool: None,
    });
    if case.regions {
        add_region_boundary(&mut session, index);
    }
    match case.curve {
        CurveModel::Fixture => {}
        CurveModel::Split => use_curve(&mut session, index, split_curve_model()),
        CurveModel::Square => use_curve(&mut session, index, square_curve_model()),
    }
    if rest {
        run_prior(&mut session);
    }
    let cancel = AtomicBool::new(false);
    match session.generate_toolpath(index, &cancel) {
        Ok(result) => Generated {
            golden: Golden::of(result.annotated()),
            unknown_intents: unknown_intents(result.toolpath()),
        },
        Err(e) => Generated {
            golden: Golden::refused(&e.to_string()),
            unknown_intents: 0,
        },
    }
}

/// The non-default arms W6.3.0 added (the `regions*` arms are found by
/// their boundary).
const W630_ARMS: &[&str] = &[
    "slope_band",
    "slope_band_hookup",
    "iso_field",
    "outside_in",
    "bottom_up",
];

/// The non-default arms W6.4.0 added: the composed-surface ops (Unified
/// Finish) and the rest detector stage (Pencil).
const W640_ARMS: &[&str] = &[
    "claims",
    "claims_rest_clip",
    "no_c2_no_hookup",
    "rest_depth",
    "rest_depth_rest",
    "curvature",
];

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
                // A field can be in one record only (W6.4.0: the optional
                // `produced` digest), so walk the union of the fields.
                let fields: BTreeSet<&String> = old_t.keys().chain(new_t.keys()).collect();
                for field in fields {
                    match (old_t.get(field), new_t.get(field)) {
                        (Some(ov), Some(nv)) if ov != nv => {
                            lines.push(format!("{key}  {field}  {ov} -> {nv}"));
                        }
                        (Some(ov), None) => lines.push(format!("{key}  {field}  {ov} -> (none)")),
                        (None, Some(nv)) => lines.push(format!("{key}  {field}  (none) -> {nv}")),
                        _ => {}
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

    // W3.4: an op with a rest rule has a rest arm, and no other op has one.
    let with_rest_rule: BTreeSet<&str> = OperationType::ALL
        .iter()
        .filter(|op| op.capabilities().rest_use != RestUse::None)
        .map(|op| op.kind_str())
        .collect();
    let rest_arms: BTreeSet<&str> = REST_READERS.iter().map(|op| op.kind_str()).collect();
    assert_eq!(
        rest_arms, with_rest_rule,
        "the rest arms are the ops whose capabilities state a rest rule"
    );

    let generated: BTreeMap<String, Generated> = cases
        .par_iter()
        .map(|case| (case.key(), run(case)))
        .collect();

    // B11: every move of every arm carries a known intent. A dressup that
    // turns an air row into a rapid tags it; it never leaves `Unknown`.
    let unknown: Vec<(&String, usize)> = generated
        .iter()
        .filter(|(_, g)| g.unknown_intents > 0)
        .map(|(k, g)| (k, g.unknown_intents))
        .collect();
    assert!(
        unknown.is_empty(),
        "these arms emit moves with MoveIntent::Unknown (arm, count): {unknown:?}"
    );

    let actual: BTreeMap<String, Golden> =
        generated.into_iter().map(|(k, g)| (k, g.golden)).collect();

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

    // W6.3.0 non-vacuity: each new arm cuts, and differs from its op's
    // fresh arm, so its switch acts. A `regions` arm also stays inside the
    // XY box of the fresh arm and is smaller on at least one side.
    for case in cases
        .iter()
        .filter(|c| c.regions || W630_ARMS.contains(&c.arm))
    {
        let key = case.key();
        let got = &actual[&key];
        let fresh = &actual[&format!("{}/fresh", case.op.kind_str())];
        assert!(got.move_count > 0, "{key}: the arm emits no moves");
        if case.source == Source::Fresh {
            assert_ne!(
                got.moves_hash, fresh.moves_hash,
                "{key}: the arm is the fresh toolpath, so its switch does nothing"
            );
        }
        if case.regions {
            let inside = (0..2).all(|i| {
                got.bbox_min[i] >= fresh.bbox_min[i] - 1e-9
                    && got.bbox_max[i] <= fresh.bbox_max[i] + 1e-9
            });
            let smaller = (0..2).any(|i| {
                got.bbox_min[i] > fresh.bbox_min[i] + 1.0
                    || got.bbox_max[i] < fresh.bbox_max[i] - 1.0
            });
            assert!(
                inside && smaller,
                "{key}: the region arm is not confined inside the fresh arm \
                 (got {:?}..{:?}, fresh {:?}..{:?})",
                got.bbox_min,
                got.bbox_max,
                fresh.bbox_min,
                fresh.bbox_max
            );
        }
    }
    // W6.4.0 non-vacuity: each new arm states its move count and whether a
    // rest detector attached its output, and a fresh-source arm differs from
    // its op's fresh arm, so its switch acts.
    for case in cases.iter().filter(|c| W640_ARMS.contains(&c.arm)) {
        let key = case.key();
        let got = &actual[&key];
        let fresh = &actual[&format!("{}/fresh", case.op.kind_str())];
        eprintln!(
            "{key}: {} moves, rest output {}",
            got.move_count,
            got.produced.is_some()
        );
        if case.source == Source::Fresh {
            assert_ne!(
                got.moves_hash, fresh.moves_hash,
                "{key}: the arm is the fresh toolpath, so its switch does nothing"
            );
        }
    }
    // W6.5.0 non-vacuity: each new arm cuts and differs from its op's fresh
    // arm, and the Project Curve link stage joins at least one gap.
    for (op, arm, ..) in W650_CASES {
        let key = format!("{}/{arm}", op.kind_str());
        let got = &actual[&key];
        let fresh = &actual[&format!("{}/fresh", op.kind_str())];
        eprintln!(
            "{key}: {} moves, {} rapids, {}",
            got.move_count, got.rapid_count, got.outcome
        );
        assert!(got.move_count > 0, "{key}: the arm emits no moves");
        assert_ne!(
            got.moves_hash, fresh.moves_hash,
            "{key}: the arm is the fresh toolpath, so its switch does nothing"
        );
    }
    assert_ne!(
        actual["project_curve/side_inside"].moves_hash, actual["project_curve/square"].moves_hash,
        "project_curve/side_inside: the side switch changes nothing on the square"
    );
    assert!(
        actual["project_curve/split_chained"].rapid_count
            < actual["project_curve/split"].rapid_count,
        "project_curve/split_chained: the link stage joins no gap, so the arm proves nothing"
    );
    // W6.4.6: the Steep and Shallow link stage acts on the rest arm.
    assert_ne!(
        actual["steep_shallow/rest_hookup"].moves_hash, actual["steep_shallow/rest"].moves_hash,
        "steep_shallow/rest_hookup: the link stage changes nothing, so the arm proves nothing"
    );
    // The link stage acts: it changes the arm it is added to.
    assert_ne!(
        actual["drop_cutter/slope_band_hookup"].moves_hash,
        actual["drop_cutter/slope_band"].moves_hash,
        "drop_cutter/slope_band_hookup: the link stage changes nothing, so the arm proves nothing"
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

// ── Instrument: the rest arms' moves and time ──────────────────────────

/// One arm of the instrument: the move count, the simulated runtime (s),
/// and the simulated entry time (s) of the case op, or `None` when the op
/// refuses.
fn timed_arm(op_type: OperationType, source: StockSource) -> Option<(usize, f64, f64)> {
    let (mut session, index) = case_session(CaseSpec {
        name: "timed",
        op: OperationConfig::new_default(op_type),
        prior: Prior::RestPrior,
        stock_source: source,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    });
    // The simulator integrates a runtime only with a kinematics block. The
    // generic block is the one `effective_kinematics` reads without it, so
    // the generated moves do not change.
    let _ = session
        .apply(rs_cam_core::session::Command::SetMachineKinematics(
            rs_cam_core::session::SetMachineKinematicsArgs {
                kinematics: Box::new(
                    rs_cam_core::machine::kinematics::MachineKinematics::generic_wood_router(),
                ),
            },
        ))
        .expect("the kinematics block");
    run_prior(&mut session);
    let cancel = AtomicBool::new(false);
    let result = session.generate_toolpath(index, &cancel).ok()?;
    let moves = result.toolpath().moves.len();
    common::op_fixtures::simulate(&mut session);
    let id = session.toolpath_configs()[index].id;
    let breakdown = session
        .simulation_result()
        .and_then(|sim| sim.cut_trace.as_deref())
        .and_then(|t| t.toolpath_runtimes.iter().find(|r| r.toolpath_id == id))
        .map(|r| r.breakdown)?;
    Some((moves, breakdown.total_s, breakdown.entry_s))
}

/// Instrument (not a gate): for each rest arm and its fresh twin (the same
/// session with the prior op, `StockSource::Fresh`), the move count, the
/// simulated runtime and the simulated entry time. Run it by name:
///
/// ```text
/// cargo test -p rs_cam_core -q --test every_op_keeps_its_toolpath_g_opgolden \
///     -- --ignored rest_arm_times --nocapture
/// ```
#[test]
#[ignore = "instrument: prints the rest arms' moves and times"]
fn rest_arm_times() {
    let rows: Vec<String> = REST_READERS
        .par_iter()
        .map(|&op_type| {
            let fmt = |a: Option<(usize, f64, f64)>| match a {
                Some((n, sim, entry)) => {
                    format!("moves {n:>6} sim {sim:>8.2} s (entry {entry:>7.2} s)")
                }
                None => "refused".to_owned(),
            };
            format!(
                "{:<16} rest: {} | fresh: {}",
                op_type.kind_str(),
                fmt(timed_arm(op_type, StockSource::FromRemainingStock)),
                fmt(timed_arm(op_type, StockSource::Fresh)),
            )
        })
        .collect();
    for row in rows {
        eprintln!("{row}");
    }
}
