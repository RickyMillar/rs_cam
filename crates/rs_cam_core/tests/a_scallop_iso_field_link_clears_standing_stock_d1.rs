//! D1 (ruling R1) — a Scallop iso-field link over the remaining stock
//! clears the standing stock.
//!
//! # The gap
//!
//! The iso field of Scallop linked its rings with its own relink
//! configuration and `link_ceiling: None` (`finish/scallop/research.rs`).
//! The registry row states `LinkCeilingNeed::WithRemainingStock`, and
//! ruling R1 says that every op that links reads the one builder
//! (`link_ceiling_for`). On a `FromRemainingStock` op material stands above
//! the mesh where no prior op cut to the surface, so a link that rides the
//! mesh is a cutting feed through it (the G-LINKLOAD class, B16).
//!
//! # The fixture
//!
//! The G-OPGOLDEN / W4 mesh fixture (`common::op_fixtures`): a spherical cap
//! on a 30 mm flat plate. Two priors:
//!
//! - `Rough`: a default 3D Rough. It leaves 0.5 mm on every cell.
//! - `RegionFinished`: the 3D Rough, then a default (contour) Scallop
//!   inside the G-OPGOLDEN region rectangle ([`REGION_RECT`]), so finished
//!   ground stands beside rough ground. The two Scallops share the tool.
//!
//! The case op is a Scallop with `iso_field = true` and the default
//! `intra_pass_hookup_mm`, on `FromRemainingStock`.
//!
//! # The claims
//!
//! 1. Population: the op makes at least one fed `Linking` move that travels
//!    in XY, so claim 2 is not vacuous.
//! 2. No sample of such a move has prior material above its tool tip on
//!    the tool axis by more than the material threshold (the W6.4.6 claim 3
//!    method).
//!
//! The test prints the count of fed links and of offending samples.
//!
//! Red proof, measured 2026-10-16 before the fix: 925 moves and 309 fed
//! link moves on each prior; 253 samples (`Rough`) and 73 samples
//! (`RegionFinished`) passed through standing stock. After the fix: 776
//! and 748 moves, 28 fed (lifted) link moves, 0 samples.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_scallop_iso_field_link_clears_standing_stock_d1
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// SAFETY: the measurement of each arm is the record of D1.
#![allow(clippy::print_stderr)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session, mesh_stock, simulate};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, StockSource,
};
use rs_cam_core::dexel_stock::{FRESH_MATERIAL_THRESHOLD_MM, TriDexelStock};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::Selection;
use rs_cam_core::session::{
    AddModelArgs, AddToolpathArgs, Command, ProjectSession, SetBoundaryConfigArgs,
};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

/// The sample step (mm) along a link move.
const SAMPLE_MM: f64 = 0.25;

/// The G-OPGOLDEN region rectangle `[x0, y0, x1, y1]` (mm).
const REGION_RECT: [f64; 4] = [-12.0, -10.0, 5.0, 9.0];

/// The float noise (mm) of an XY travel.
const LENGTH_EPS_MM: f64 = 1e-6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PriorKind {
    Rough,
    RegionFinished,
}

fn iso_field_scallop() -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::Scallop);
    let OperationConfig::Scallop(cfg) = &mut op else {
        unreachable!("a Scallop config")
    };
    cfg.iso_field = true;
    assert!(
        cfg.intra_pass_hookup_mm > 0.0,
        "the default hookup is on, so the iso field links"
    );
    op
}

/// Put the [`REGION_RECT`] boundary on toolpath `index`.
fn add_region_boundary(session: &mut ProjectSession, index: usize) {
    let [x0, y0, x1, y1] = REGION_RECT;
    let model = common::session::polygon_model(
        vec![common::op_fixtures::rectangle(x0, y0, x1, y1)],
        "d1_region",
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
                source: BoundarySource::Selection {
                    selection: Selection::closed_shapes_of(ModelId(model_id)),
                    holes: false,
                },
                containment: BoundaryContainment::Center,
                offset: 0.0,
            },
        }))
        .expect("set the region boundary");
}

fn last_stock(session: &ProjectSession) -> Arc<TriDexelStock> {
    session
        .simulation_result()
        .and_then(|sim| sim.checkpoints.last().map(|c| Arc::clone(&c.mesh_stock)))
        .expect("the simulation holds a checkpoint")
}

/// The case op over `prior`: its toolpath and the stock before it.
fn arm(prior: PriorKind) -> (Toolpath, Arc<TriDexelStock>) {
    let cancel = AtomicBool::new(false);
    let (mut session, index) = match prior {
        PriorKind::Rough => case_session(CaseSpec {
            name: "scallop_iso_field_d1",
            op: iso_field_scallop(),
            prior: Prior::RestPrior,
            stock_source: StockSource::FromRemainingStock,
            frame: Frame::TopUp,
            fixture: Fixture::Standard,
            tool: None,
        }),
        PriorKind::RegionFinished => {
            let (mut session, finish) = case_session(CaseSpec {
                name: "region_finish",
                op: OperationConfig::new_default(OperationType::Scallop),
                prior: Prior::RestPrior,
                stock_source: StockSource::Fresh,
                frame: Frame::TopUp,
                fixture: Fixture::Standard,
                tool: None,
            });
            add_region_boundary(&mut session, finish);
            let mut config = session.toolpath_configs()[finish].clone();
            config.name = "scallop_iso_field_d1".to_owned();
            config.operation = iso_field_scallop();
            config.boundary = BoundaryConfig::default();
            config.stock_source = StockSource::FromRemainingStock;
            let index = session
                .apply(Command::AddToolpath(AddToolpathArgs {
                    setup_index: 0,
                    config: Box::new(config),
                }))
                .expect("add the case op")
                .created
                .expect("the add reports the index");
            (session, index)
        }
    };
    for p in 0..index {
        let _ = session
            .generate_toolpath(p, &cancel)
            .expect("the prior op generates");
    }
    simulate(&mut session);
    let before = last_stock(&session);
    let toolpath = session
        .generate_toolpath(index, &cancel)
        .expect("the op generates")
        .toolpath()
        .clone();
    (toolpath, before)
}

/// The top (mm) of a stock cell; an empty column reads as the stock floor.
fn top(s: &TriDexelStock, r: usize, c: usize) -> f64 {
    f64::from(
        s.z_grid
            .top_z_at(r, c)
            .unwrap_or(mesh_stock().origin_z as f32),
    )
}

/// The samples of the straight move `a -> b`, one per [`SAMPLE_MM`].
fn samples(a: P3, b: P3) -> impl Iterator<Item = P3> {
    let d = b - a;
    let n = (d.norm() / SAMPLE_MM).ceil().max(1.0) as usize;
    (0..n).map(move |k| a + d * ((k as f64 + 0.5) / n as f64))
}

/// The count of fed link moves that travel in XY, and the samples of them
/// with prior material above the tool tip.
fn link_offenders(tp: &Toolpath, prior: &TriDexelStock) -> (usize, Vec<(P3, f64)>) {
    let g = &prior.z_grid;
    let mut links = 0;
    let mut offenders = Vec::new();
    let mut prev: Option<P3> = None;
    for m in &tp.moves {
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
                let t = top(prior, row, col);
                if t > p.z + FRESH_MATERIAL_THRESHOLD_MM {
                    offenders.push((p, t));
                }
            }
        }
        prev = Some(m.target);
    }
    (links, offenders)
}

fn claims(prior: PriorKind) {
    let (tp, before) = arm(prior);
    let (links, offenders) = link_offenders(&tp, &before);
    eprintln!(
        "{prior:?} prior: {} moves, {links} fed link moves, {} samples through standing stock",
        tp.moves.len(),
        offenders.len()
    );
    assert!(
        links > 0,
        "{prior:?}: the iso field made no fed link, so the claim below proves nothing"
    );
    assert!(
        offenders.is_empty(),
        "{prior:?}: {} fed link samples have prior material above the tool tip (a cutting \
         feed through standing stock, G-LINKLOAD); first {:?}",
        offenders.len(),
        offenders.first()
    );
}

#[test]
fn over_a_rough_prior_an_iso_field_link_cuts_no_standing_stock() {
    claims(PriorKind::Rough);
}

#[test]
fn over_a_region_finished_prior_an_iso_field_link_cuts_no_standing_stock() {
    claims(PriorKind::RegionFinished);
}
