//! W6.4.6 (ruling R-W6.4b) — Steep and Shallow over the remaining stock
//! cuts only where material stands, and its links clear the stock.
//!
//! # The gap
//!
//! Until W6.4.6 Steep and Shallow had `rest_use: None` and no link stage:
//! on `FromRemainingStock` it planned the whole surface again, and only the
//! session's air-cut dressup took out the rows in air.
//!
//! # The rule now
//!
//! `RestUse::ContourFilter`, the Waterline rule (W6.1): each point of both
//! halves stays when material stands above its tool tip within `R + cell`
//! (`standing_material::standing_near`). A stretch in air between two kept
//! points stays when the feed through it is faster than a retract. The
//! `hookup_mm` link stage reads the one link ceiling (`link_ceiling_for`).
//!
//! # The fixtures
//!
//! The G-OPGOLDEN / W4 mesh fixture (`common::op_fixtures`): a spherical cap
//! on a 30 mm flat plate. Three priors:
//!
//! - `Rough`: a default 3D Rough. It leaves 0.5 mm on every cell, so a
//!   correct rest rule drops almost nothing here.
//! - `RegionFinished`: the 3D Rough, then a default Steep and Shallow
//!   inside the G-OPGOLDEN region rectangle ([`REGION_RECT`]): the rest op
//!   meets finished and unfinished ground.
//! - `Finished`: the long plate ([`long_session`]), the 3D Rough, then a
//!   default Steep and Shallow over the whole surface.
//!
//! On each prior the op runs as `Fresh` (it ignores the prior cut) and as
//! `FromRemainingStock`, in two sessions. Both simulate at `SIM_CELL_MM`.
//!
//! # The claims
//!
//! 1. The rest op removes the same material as the Fresh op: its removed
//!    volume is no less than the Fresh volume less the material threshold
//!    over each cell the Fresh op lowered.
//! 2. The rest op cuts no more than the Fresh op, and no more air (cutting
//!    length with no prior material in the tool disc). On the `Finished`
//!    prior it cuts strictly less of both. The unit test
//!    `the_rest_rule_confines_both_halves_to_the_standing_material`
//!    (`finish::steep_shallow`) proves that the op rule itself acts; the
//!    session's air-cut dressup alone also cuts less here.
//! 3. On the `RegionFinished` prior with the link stage on (`hookup_mm`),
//!    the rest op links at least one junction with a fed `Linking` move
//!    that travels in XY, and no sample of such a move has prior material
//!    above its tool tip on the tool axis (the B16 approach). Red proof,
//!    done once by hand: with no link ceiling, 13 samples fail.
//!
//! The test prints the measurement of each arm.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test steep_shallow_cuts_only_the_rest_w6_4_6
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// SAFETY: the measurement of each arm is the record of W6.4.6.
#![allow(clippy::print_stderr)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::op_fixtures::{
    CaseSpec, Fixture, Frame, Prior, case_session, mesh_stock, run_prior, simulate, tool_for,
};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, StockSource,
};
use rs_cam_core::dexel_stock::{FRESH_MATERIAL_THRESHOLD_MM, TriDexelStock};
use rs_cam_core::geo::P3;
use rs_cam_core::machine::kinematics::compute_cycle_time;
use rs_cam_core::session::{
    AddModelArgs, AddToolpathArgs, Command, ProjectSession, SetBoundaryConfigArgs,
};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};

/// The sample step (mm) along a move for the air and link measurements.
const SAMPLE_MM: f64 = 0.25;

/// The G-OPGOLDEN region rectangle `[x0, y0, x1, y1]` (mm). It cuts
/// through the cap (foot radius about 9.2 mm).
const REGION_RECT: [f64; 4] = [-12.0, -10.0, 5.0, 9.0];

/// The link-stage cap (mm) of the hookup arm: the G-OPGOLDEN
/// `rest_hookup` value.
const HOOKUP_MM: f64 = 10.0;

/// The float noise (mm) of two length sums.
const LENGTH_EPS_MM: f64 = 1e-6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PriorKind {
    Rough,
    /// The rough, then a Steep and Shallow inside [`REGION_RECT`].
    RegionFinished,
    /// The long plate ([`long_session`]): the rough, then a Steep and
    /// Shallow over the whole surface.
    Finished,
}

/// Length (mm) along X of the long plate, and its width along Y.
const LONG_X: f64 = 80.0;
const LONG_Y: f64 = 30.0;
/// The X (mm) of the cap centre on the long plate: near one end, so most
/// raster rows cross a long stretch of flat ground.
const LONG_CAP_X: f64 = -22.0;

/// The long plate: the G-OPGOLDEN cap on a flat plate of `LONG_X` by
/// `LONG_Y`, the stock over it, and three toolpaths: a default 3D Rough, a
/// default Steep and Shallow, and the case op (index 2). On the
/// 30 mm G-OPGOLDEN plate each flat stretch of a row is shorter than the
/// feed a retract replaces, so the time rule feeds through it; here the
/// stretches beyond the cap are long enough for a retract to win.
fn long_session(op: OperationConfig, source: StockSource) -> (ProjectSession, usize) {
    use common::op_fixtures::{MESH_STOCK_Z, SPHERE_CZ, SPHERE_R};
    use common::session::{mesh_model, pinned_heights, toolpath_config};
    use rs_cam_core::compute::StockConfig;
    use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use rs_cam_core::session::{ProjectSessionBuilder, ToolpathConfig};

    let step = common::op_fixtures::MESH_STEP;
    let mesh = common::meshes::height_field_grid(
        -LONG_X / 2.0,
        step,
        (LONG_X / step).round() as usize + 1,
        -LONG_Y / 2.0,
        step,
        (LONG_Y / step).round() as usize + 1,
        |x, y| {
            let dx = x - LONG_CAP_X;
            let s = SPHERE_R * SPHERE_R - dx * dx - y * y;
            if s <= 0.0 {
                0.0
            } else {
                (s.sqrt() + SPHERE_CZ).max(0.0)
            }
        },
    );
    let mut builder = ProjectSessionBuilder::new().stock(StockConfig {
        x: LONG_X,
        y: LONG_Y,
        z: MESH_STOCK_Z,
        origin_x: -LONG_X / 2.0,
        origin_y: -LONG_Y / 2.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let tool = builder.add_tool(tool_for(OperationType::SteepShallow));
    let tool_id = builder.tools()[tool].id.0;
    let rough_tool = builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    let rough_tool_id = builder.tools()[rough_tool].id.0;
    let model_id = builder.add_model(mesh_model(mesh, "w6_4_6_long_plate"));
    let heights = || pinned_heights(MESH_STOCK_Z, 0.0);
    let configs = [
        ToolpathConfig {
            heights: heights(),
            ..toolpath_config(
                "rough",
                OperationConfig::new_default(OperationType::Adaptive3d),
                rough_tool_id,
                model_id,
            )
        },
        ToolpathConfig {
            heights: heights(),
            ..toolpath_config("finish", steep_shallow(0.0), tool_id, model_id)
        },
        ToolpathConfig {
            heights: heights(),
            stock_source: source,
            ..toolpath_config("steep_shallow_w6_4_6", op, tool_id, model_id)
        },
    ];
    let mut index = 0;
    for config in configs {
        index = builder.add_toolpath(0, config).expect("add a toolpath");
    }
    (builder.build(), index)
}

fn last_stock(session: &ProjectSession) -> Arc<TriDexelStock> {
    session
        .simulation_result()
        .and_then(|sim| sim.checkpoints.last().map(|c| Arc::clone(&c.mesh_stock)))
        .expect("the simulation holds a checkpoint")
}

/// One arm: the op after the prior, with its source.
struct Arm {
    toolpath: Toolpath,
    prior: Arc<TriDexelStock>,
    last: Arc<TriDexelStock>,
}

fn steep_shallow(hookup_mm: f64) -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::SteepShallow);
    let OperationConfig::SteepShallow(cfg) = &mut op else {
        unreachable!("a Steep and Shallow config")
    };
    cfg.hookup_mm = hookup_mm;
    op
}

/// Put the [`REGION_RECT`] boundary on toolpath `index`.
fn add_region_boundary(session: &mut ProjectSession, index: usize) {
    let [x0, y0, x1, y1] = REGION_RECT;
    let model = common::session::polygon_model(
        vec![common::op_fixtures::rectangle(x0, y0, x1, y1)],
        "w6_4_6_region",
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

fn arm(prior: PriorKind, source: StockSource, hookup_mm: f64) -> Arm {
    let cancel = AtomicBool::new(false);
    let (mut session, index) = match prior {
        PriorKind::Rough => {
            let (mut session, index) = case_session(CaseSpec {
                name: "steep_shallow_w6_4_6",
                op: steep_shallow(hookup_mm),
                prior: Prior::RestPrior,
                stock_source: source,
                frame: Frame::TopUp,
                fixture: Fixture::Standard,
                tool: None,
            });
            run_prior(&mut session);
            (session, index)
        }
        PriorKind::Finished => {
            let (mut session, index) = long_session(steep_shallow(hookup_mm), source);
            for prior in 0..index {
                let _ = session
                    .generate_toolpath(prior, &cancel)
                    .expect("the prior op generates");
            }
            simulate(&mut session);
            (session, index)
        }
        PriorKind::RegionFinished => {
            // Toolpath 1 is the region finish; the case op joins after it.
            let (mut session, finish) = case_session(CaseSpec {
                name: "region_finish",
                op: steep_shallow(0.0),
                prior: Prior::RestPrior,
                stock_source: StockSource::Fresh,
                frame: Frame::TopUp,
                fixture: Fixture::Standard,
                tool: None,
            });
            add_region_boundary(&mut session, finish);
            let mut config = session.toolpath_configs()[finish].clone();
            config.name = "steep_shallow_w6_4_6".to_owned();
            config.operation = steep_shallow(hookup_mm);
            config.boundary = BoundaryConfig::default();
            config.stock_source = source;
            let index = session
                .apply(Command::AddToolpath(AddToolpathArgs {
                    setup_index: 0,
                    config: Box::new(config),
                }))
                .expect("add the case op")
                .created
                .expect("the add reports the index");
            for prior in 0..=finish {
                let _ = session
                    .generate_toolpath(prior, &cancel)
                    .expect("the prior op generates");
            }
            simulate(&mut session);
            (session, index)
        }
    };
    let prior = last_stock(&session);
    let toolpath = session
        .generate_toolpath(index, &cancel)
        .expect("the op generates")
        .toolpath()
        .clone();
    simulate(&mut session);
    Arm {
        toolpath,
        prior,
        last: last_stock(&session),
    }
}

fn is_cut(m: &Move) -> bool {
    m.move_type != MoveType::Rapid
        && matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut)
}

/// The top (mm) of a stock cell; an empty column reads as the stock floor.
fn top(s: &TriDexelStock, r: usize, c: usize) -> f64 {
    f64::from(
        s.z_grid
            .top_z_at(r, c)
            .unwrap_or(mesh_stock().origin_z as f32),
    )
}

/// The volume (mm³) `last` removed from `prior`.
fn removed(prior: &TriDexelStock, last: &TriDexelStock) -> f64 {
    let g = &prior.z_grid;
    let mut v = 0.0;
    for r in 0..g.rows {
        for c in 0..g.cols {
            v += (top(prior, r, c) - top(last, r, c)).max(0.0);
        }
    }
    v * g.cell_size * g.cell_size
}

/// The count of cells `last` lowered below `prior` by more than the
/// material threshold.
fn lowered_cells(prior: &TriDexelStock, last: &TriDexelStock) -> usize {
    let g = &prior.z_grid;
    let mut n = 0;
    for r in 0..g.rows {
        for c in 0..g.cols {
            if top(last, r, c) < top(prior, r, c) - FRESH_MATERIAL_THRESHOLD_MM {
                n += 1;
            }
        }
    }
    n
}

/// True when no prior material stands within `radius` of `p` above
/// `p.z + FRESH_MATERIAL_THRESHOLD_MM`: the flat tool there cuts air.
fn in_air(prior: &TriDexelStock, p: P3, radius: f64) -> bool {
    let g = &prior.z_grid;
    let span = (radius / g.cell_size).ceil() as isize + 1;
    let Some((row, col)) = g.world_to_cell(p.x, p.y) else {
        return true;
    };
    for dr in -span..=span {
        for dc in -span..=span {
            let (rr, cc) = (row as isize + dr, col as isize + dc);
            if rr < 0 || cc < 0 || rr as usize >= g.rows || cc as usize >= g.cols {
                continue;
            }
            let (x, y) = g.cell_to_world(rr as usize, cc as usize);
            if (x - p.x).hypot(y - p.y) > radius {
                continue;
            }
            if top(prior, rr as usize, cc as usize) > p.z + FRESH_MATERIAL_THRESHOLD_MM {
                return false;
            }
        }
    }
    true
}

/// The samples of the straight move `a -> b`, one per [`SAMPLE_MM`], at
/// the middle of each step.
fn samples(a: P3, b: P3) -> impl Iterator<Item = P3> {
    let d = b - a;
    let n = (d.norm() / SAMPLE_MM).ceil().max(1.0) as usize;
    (0..n).map(move |k| a + d * ((k as f64 + 0.5) / n as f64))
}

/// The cutting length (mm) and the part of it in air. An arc counts as
/// its chord.
fn cut_and_air(tp: &Toolpath, prior: &TriDexelStock, radius: f64) -> (f64, f64) {
    let (mut cut, mut air) = (0.0, 0.0);
    let mut prev: Option<P3> = None;
    for m in &tp.moves {
        if is_cut(m)
            && let Some(a) = prev
        {
            let len = (m.target - a).norm();
            cut += len;
            let n = (len / SAMPLE_MM).ceil().max(1.0);
            air += samples(a, m.target)
                .filter(|&p| in_air(prior, p, radius))
                .count() as f64
                * len
                / n;
        }
        prev = Some(m.target);
    }
    (cut, air)
}

struct Measure {
    moves: usize,
    rapids: usize,
    cut_mm: f64,
    air_mm: f64,
    time_s: f64,
    removed_mm3: f64,
    lowered: usize,
}

fn measure(a: &Arm, radius: f64) -> Measure {
    let (cut_mm, air_mm) = cut_and_air(&a.toolpath, &a.prior, radius);
    let lk = rs_cam_core::machine::MachineProfile::default().link_kinematics();
    Measure {
        moves: a.toolpath.moves.len(),
        rapids: a
            .toolpath
            .moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid)
            .count(),
        cut_mm,
        air_mm,
        time_s: compute_cycle_time(
            &a.toolpath,
            &lk.kinematics,
            lk.max_feed_mm_min,
            lk.rapid_feed_mm_min,
        ),
        removed_mm3: removed(&a.prior, &a.last),
        lowered: lowered_cells(&a.prior, &a.last),
    }
}

fn print(name: &str, m: &Measure) {
    eprintln!(
        "{name}: {} moves, {} rapids, cut {:.1} mm, air {:.1} mm, time {:.2} s, \
         removed {:.1} mm3 over {} cells",
        m.moves, m.rapids, m.cut_mm, m.air_mm, m.time_s, m.removed_mm3, m.lowered
    );
}

/// The most material (mm) the rest stock holds above the Fresh stock.
fn worst_left(fresh: &Arm, rest: &Arm) -> f64 {
    let g = &fresh.last.z_grid;
    (0..g.rows)
        .flat_map(|row| (0..g.cols).map(move |col| (row, col)))
        .map(|(row, col)| top(&rest.last, row, col) - top(&fresh.last, row, col))
        .fold(0.0_f64, f64::max)
}

/// Claims 1 and 2 on one prior.
fn rest_claims(prior: PriorKind) {
    let radius = tool_for(OperationType::SteepShallow).diameter / 2.0;
    let fresh = arm(prior, StockSource::Fresh, 0.0);
    let rest = arm(prior, StockSource::FromRemainingStock, 0.0);
    let (f, r) = (measure(&fresh, radius), measure(&rest, radius));
    print(&format!("{prior:?} prior, fresh source"), &f);
    print(&format!("{prior:?} prior, rest source"), &r);
    eprintln!(
        "{prior:?} prior: rest stock over the Fresh stock, worst {:.3} mm",
        worst_left(&fresh, &rest)
    );

    let g = &fresh.prior.z_grid;
    let slack = f.lowered as f64 * g.cell_size * g.cell_size * FRESH_MATERIAL_THRESHOLD_MM;
    assert!(
        r.removed_mm3 >= f.removed_mm3 - slack,
        "{prior:?}: the rest op removes {:.1} mm3, the Fresh op {:.1} mm3 (slack {slack:.1}); \
         a rest rule must not leave material the Fresh op removes",
        r.removed_mm3,
        f.removed_mm3
    );
    assert!(
        r.cut_mm <= f.cut_mm + LENGTH_EPS_MM && r.air_mm <= f.air_mm + LENGTH_EPS_MM,
        "{prior:?}: the rest op cuts {:.1} mm ({:.1} mm air), the Fresh op {:.1} mm \
         ({:.1} mm air); a rest rule only drops runs",
        r.cut_mm,
        r.air_mm,
        f.cut_mm,
        f.air_mm
    );
    if prior == PriorKind::Finished {
        assert!(
            r.cut_mm < f.cut_mm && r.air_mm < f.air_mm,
            "Finished: the prior finished part of the surface, so the rest op must cut \
             less ({:.1} vs {:.1} mm) and less air ({:.1} vs {:.1} mm)",
            r.cut_mm,
            f.cut_mm,
            r.air_mm,
            f.air_mm
        );
    }
}

#[test]
fn over_a_rough_prior_the_rest_op_removes_what_the_fresh_op_removes() {
    rest_claims(PriorKind::Rough);
}

#[test]
fn over_a_region_finished_prior_the_rest_op_removes_what_the_fresh_op_removes() {
    rest_claims(PriorKind::RegionFinished);
}

#[test]
fn over_a_finished_prior_the_rest_op_cuts_less_and_removes_the_same() {
    rest_claims(PriorKind::Finished);
}

/// Claim 3: the fed links of the rest op clear the prior stock.
#[test]
fn a_rest_link_cuts_no_standing_stock() {
    // The region prior: rough stock stands beside finished ground, so a
    // link that rides the surface there feeds through standing stock.
    let rest = arm(
        PriorKind::RegionFinished,
        StockSource::FromRemainingStock,
        HOOKUP_MM,
    );
    let radius = tool_for(OperationType::SteepShallow).diameter / 2.0;
    print(
        "RegionFinished prior, rest source, hookup",
        &measure(&rest, radius),
    );
    let g = &rest.prior.z_grid;
    let mut links = 0;
    let mut offenders = Vec::new();
    let mut prev: Option<P3> = None;
    for m in &rest.toolpath.moves {
        // B16: a leg that travels in XY. A vertical rise out of the cut
        // removes no material.
        if m.move_type != MoveType::Rapid
            && m.intent == MoveIntent::Linking
            && let Some(a) = prev
            && (m.target.x - a.x).hypot(m.target.y - a.y) > LENGTH_EPS_MM
        {
            links += 1;
            for p in samples(a, m.target) {
                let Some((row, col)) = g.world_to_cell(p.x, p.y) else {
                    continue;
                };
                let t = top(&rest.prior, row, col);
                if t > p.z + FRESH_MATERIAL_THRESHOLD_MM {
                    offenders.push((p, t));
                }
            }
        }
        prev = Some(m.target);
    }
    eprintln!("hookup arm: {links} fed link moves");
    assert!(
        links > 0,
        "the link stage made no fed link, so the claim below proves nothing"
    );
    assert!(
        offenders.is_empty(),
        "{} fed link samples have prior material above the tool tip (a cutting feed \
         through standing stock, G-LINKLOAD); first {:?}",
        offenders.len(),
        offenders.first()
    );
}
