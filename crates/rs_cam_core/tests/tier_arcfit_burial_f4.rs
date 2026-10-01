//! Tiered-finish plan F4 (`planning/tiered_finish_2026-09-30/PLAN.md`): does
//! the arc-fit dressup on a tier op bury the tool below the surface by more
//! than the source polyline already does?
//!
//! # The mechanism under test
//!
//! `dressup::arcfit` accepts a run when every point is within the arc
//! tolerance of the circle in XY and within the tolerance of the helical Z
//! interpolation. The Finish role turns arcs on at 0.05 mm, and until F4a
//! (2026-10-01) the planner's tier ops inherited that role's dressups; they
//! now carry cusp / 2 (`tests/tier_ops_fit_arcs_within_half_the_cusp_f4a.rs`).
//! At 0.05 an arc can pass up to 0.05 mm
//! into a wall in XY and 0.05 mm below the source in Z, against a planned
//! cusp of 0.03 mm. The competing source is the sag of the source polyline
//! itself, which is present with arcs off.
//!
//! # The fixture
//!
//! A 40 x 40 mm plane with a 60° wall rising to a plateau, and spherical-cap
//! knolls on the plane (height 0.3 mm = 10 x the planned cusp, base radius
//! 0.4 mm). The wall runs round a circular mesa: a straight wall gives
//! straight waterline contours, which the fitter never turns into arcs, and
//! the mechanism under test is an arc that passes into a steep wall. The ladder is a Ø4 ball over a Ø2 ball (R1.0); the planner emits
//! the tiers, and the fine tier's op is generated over the whole board with
//! its boundary off and stock Fresh, so only the dressup changes between
//! arms:
//!
//! - A: arcs at 0.05 mm (as the planner emitted before F4a);
//! - B: arcs off;
//! - C: arcs at 0.015 mm (cusp height / 2, as the planner emits since F4a).
//!
//! Primary measure: `dressup::entry_audit::buried_fed_chords` on every fed
//! move, 0.05 mm samples, floor = the fine tool's drop-cutter surface.
//! Secondary: a 0.2 mm simulation, `column_deviations`, low tail in a
//! knoll/wall mask.
//!
//! The sentry `an_arc_fit_buries_no_deeper_than_the_source_path_plus_its_tolerance`
//! runs in the gate (three generations, no simulation). The two instruments
//! (this fixture with the simulation, and the rivmap100 fine tier) run with
//! `cargo test -p rs_cam_core --test tier_arcfit_burial_f4 -- --ignored
//! --nocapture`. Results: `planning/tiered_finish_2026-09-30/RESULTS.md`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::session::{mesh_model, stock_over};
use common::tools::ball_tool_config;
use rs_cam_core::compute::config::{ArcFitParams, BoundaryConfig, StockSource};
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::dressup::entry_audit::{BuriedChord, buried_fed_chords};
use rs_cam_core::dressup::{EntrySurfaceProbe, OffMeshEntry};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::session::{
    Command, MultitoolPlanSpec, ProjectSession, ProjectSessionBuilder, SetBoundaryConfigArgs,
    SetDressupConfigArgs, SetStockSourceArgs, SimulationOptions,
};
use rs_cam_core::toolpath::{MoveType, Toolpath};

const HALF: f64 = 20.0;
/// Mesh step (mm): a knoll base of 0.4 mm spans eight samples.
const STEP: f64 = 0.1;
const KNOLL_H: f64 = 0.3;
const KNOLL_R: f64 = 0.4;
const KNOLL_PITCH: f64 = 3.0;
/// Knolls stand on the plane at x <= this (mm).
const KNOLL_X_MAX: f64 = -2.0;
/// Mesa centre X (mm, Y = 0) and its plateau radius.
const MESA_X: f64 = 9.0;
const MESA_TOP_R: f64 = 5.0;
const WALL_H: f64 = 6.0;
const WALL_DEG: f64 = 60.0;
const FINE_DIAMETER: f64 = 2.0;
const COARSE_DIAMETER: f64 = 4.0;
/// Burial samples along each fed move (mm), the plan's spacing.
const SAMPLE_MM: f64 = 0.05;
const SIM_CELL_MM: f64 = 0.2;

/// The mesa's foot radius: the plateau radius plus the flank's run.
fn mesa_foot_r() -> f64 {
    MESA_TOP_R + WALL_H / WALL_DEG.to_radians().tan()
}

fn mesa_rho(x: f64, y: f64) -> f64 {
    ((x - MESA_X).powi(2) + y * y).sqrt()
}

/// The cap's sphere radius: a cap of height h over base radius r.
fn knoll_sphere_r() -> f64 {
    (KNOLL_R * KNOLL_R + KNOLL_H * KNOLL_H) / (2.0 * KNOLL_H)
}

/// The nearest knoll centre to (x, y), or `None` off the knoll field.
fn nearest_knoll(x: f64, y: f64) -> Option<(f64, f64)> {
    let cx = (x / KNOLL_PITCH).round() * KNOLL_PITCH;
    let cy = (y / KNOLL_PITCH).round() * KNOLL_PITCH;
    (cx <= KNOLL_X_MAX && cx.abs() < HALF - 1.0 && cy.abs() < HALF - 1.0).then_some((cx, cy))
}

fn surface_z(x: f64, y: f64) -> f64 {
    mesa_z(mesa_rho(x, y)) + knoll_z(x, y)
}

/// The mesa alone at radius `rho` from its axis.
fn mesa_z(rho: f64) -> f64 {
    if rho <= MESA_TOP_R {
        WALL_H
    } else if rho < mesa_foot_r() {
        WALL_H - (rho - MESA_TOP_R) * WALL_DEG.to_radians().tan()
    } else {
        0.0
    }
}

/// The knoll field's height at (x, y).
fn knoll_z(x: f64, y: f64) -> f64 {
    nearest_knoll(x, y).map_or(0.0, |(cx, cy)| {
        let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
        if d < KNOLL_R {
            let rs = knoll_sphere_r();
            KNOLL_H - rs + (rs * rs - d * d).sqrt()
        } else {
            0.0
        }
    })
}

fn mesh() -> TriangleMesh {
    height_field(HALF, STEP, surface_z)
}

/// Half-width (mm) of the sentry's board: the mesa alone, centred, no
/// knolls. The instrument's 40 mm board with knolls generates three arms in
/// about three minutes in a debug build; the arcs the sentry is about sit on
/// the mesa wall.
const SENTRY_HALF: f64 = 11.0;

fn sentry_mesh() -> TriangleMesh {
    height_field(SENTRY_HALF, STEP, |x, y| mesa_z((x * x + y * y).sqrt()))
}

/// The knoll/wall mask for the secondary measure: within a knoll's base plus
/// half the fine ball's CL bump width, or within 1 mm of the wall band.
fn in_mask(x: f64, y: f64) -> bool {
    let r = FINE_DIAMETER / 2.0;
    let half_bump = (2.0 * r * KNOLL_H - KNOLL_H * KNOLL_H).sqrt();
    let near_knoll = nearest_knoll(x, y)
        .is_some_and(|(cx, cy)| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() < KNOLL_R + half_bump);
    let rho = mesa_rho(x, y);
    let near_wall = rho > MESA_TOP_R - 1.0 && rho < mesa_foot_r() + 1.0;
    near_knoll || near_wall
}

struct Burial {
    max_all: f64,
    max_arc: f64,
    max_linear: f64,
    over_0_005: usize,
    over_0_015: usize,
    over_0_05: usize,
}

fn burial(tp: &Toolpath, mesh: &TriangleMesh, index: &SpatialIndex) -> Burial {
    let cutter = build_cutter(&ball_tool_config(FINE_DIAMETER));
    let probe = EntrySurfaceProbe {
        mesh,
        index,
        cutter: &cutter,
        stock_to_leave: 0.0,
        off_mesh: OffMeshEntry::PlungeFallback,
        rest_stock: None,
    };
    let reports = buried_fed_chords(tp, &probe, SAMPLE_MM, f64::NEG_INFINITY, |_| true);
    summarise(tp, &reports)
}

/// Max burial over all fed moves, split arc / linear, and the counts over
/// three depths.
fn summarise(tp: &Toolpath, reports: &[BuriedChord]) -> Burial {
    let mut b = Burial {
        max_all: 0.0,
        max_arc: 0.0,
        max_linear: 0.0,
        over_0_005: 0,
        over_0_015: 0,
        over_0_05: 0,
    };
    for r in reports {
        let depth = r.max_burial_mm;
        b.max_all = b.max_all.max(depth);
        match tp.moves[r.move_index].move_type {
            MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => b.max_arc = b.max_arc.max(depth),
            _ => b.max_linear = b.max_linear.max(depth),
        }
        b.over_0_005 += usize::from(depth > 0.005);
        b.over_0_015 += usize::from(depth > 0.015);
        b.over_0_05 += usize::from(depth > 0.05);
    }
    b
}

/// Burial per SAMPLE, not per move: every fed move sampled at
/// [`SAMPLE_MM`] (arcs along the arc), each sample's depth below the floor.
/// A per-move maximum cannot compare arms fairly, because an arc replaces
/// many linear moves; path length buried past a depth can.
#[derive(Default)]
struct SampleBurial {
    /// Samples on arc moves / on linear `FinishingCut` moves / on the rest.
    n: [usize; 3],
    /// Per class: samples deeper than 0.005 / 0.015 / 0.05 / 0.1 mm.
    over: [[usize; 4]; 3],
    /// Per class: the deepest sample.
    max: [f64; 3],
}

const SAMPLE_DEPTHS: [f64; 4] = [0.005, 0.015, 0.05, 0.1];

fn sample_burial(tp: &Toolpath, probe: &EntrySurfaceProbe<'_>) -> SampleBurial {
    use rs_cam_core::geo::P3;
    use rs_cam_core::geometry::arc_util::linearize_arc;
    use rs_cam_core::toolpath::MoveIntent;

    let mut out = SampleBurial::default();
    for pair in tp.moves.windows(2) {
        let (prev, m) = (&pair[0], &pair[1]);
        let (class, samples): (usize, Vec<P3>) = match m.move_type {
            MoveType::Rapid => continue,
            MoveType::ArcCW { i, j, .. } => (
                0,
                linearize_arc(prev.target, m.target, i, j, true, SAMPLE_MM),
            ),
            MoveType::ArcCCW { i, j, .. } => (
                0,
                linearize_arc(prev.target, m.target, i, j, false, SAMPLE_MM),
            ),
            MoveType::Linear { .. } => {
                let (a, b) = (prev.target, m.target);
                let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
                let n = (len / SAMPLE_MM).ceil().max(1.0) as usize;
                let pts = (0..=n)
                    .map(|k| {
                        let t = k as f64 / n as f64;
                        P3::new(
                            a.x + (b.x - a.x) * t,
                            a.y + (b.y - a.y) * t,
                            a.z + (b.z - a.z) * t,
                        )
                    })
                    .collect();
                let class = if m.intent == MoveIntent::FinishingCut {
                    1
                } else {
                    2
                };
                (class, pts)
            }
        };
        // The first sample is the previous move's end, counted there.
        for p in samples.iter().skip(1) {
            let Some(floor) = probe.floor_z(p.x, p.y) else {
                continue;
            };
            let depth = floor - p.z;
            out.n[class] += 1;
            out.max[class] = out.max[class].max(depth);
            for (slot, &d) in out.over[class].iter_mut().zip(SAMPLE_DEPTHS.iter()) {
                *slot += usize::from(depth > d);
            }
        }
    }
    out
}

/// Links and cycle time of one generated tier op: fed `Linking` moves,
/// rapid runs (one per air link), and the F-034 cycle time with the
/// machine's kinematics and rapid = max feed (as
/// `tier_fine_burial_sources_g_tierburial` reads it).
fn print_links_and_time(label: &str, session: &ProjectSession, tp: &Toolpath) {
    use rs_cam_core::toolpath::MoveIntent;

    let fed_links = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Linking && !matches!(m.move_type, MoveType::Rapid))
        .count();
    let rapid_runs = tp
        .moves
        .windows(2)
        .filter(|w| {
            matches!(w[1].move_type, MoveType::Rapid) && !matches!(w[0].move_type, MoveType::Rapid)
        })
        .count();
    let machine = session.machine();
    let cycle_s = rs_cam_core::machine::kinematics::compute_cycle_time(
        tp,
        &machine.kinematics.unwrap_or_default(),
        machine.max_feed_mm_min,
        machine.max_feed_mm_min.max(1.0),
    );
    eprintln!(
        "    {label:>6} links: {fed_links} fed Linking moves, {rapid_runs} rapid runs; \
         cycle time {cycle_s:.1} s"
    );
}

fn print_sample_burial(label: &str, b: &SampleBurial) {
    for (class, name) in ["arc", "cut", "other"].iter().enumerate() {
        eprintln!(
            "    {label:>6} {name:>5}: {:>8} samples, max {:>7.4} mm, over .005/.015/.05/.1: \
             {:>7} {:>7} {:>7} {:>7}",
            b.n[class],
            b.max[class],
            b.over[class][0],
            b.over[class][1],
            b.over[class][2],
            b.over[class][3],
        );
    }
}

/// The session with the planner's tier chain, and the index of the fine
/// tier, unconfined and on fresh stock.
fn session_with_fine_tier(mesh: TriangleMesh, half: f64) -> (ProjectSession, usize) {
    let mut builder = ProjectSessionBuilder::new().stock(stock_over(half, WALL_H + 2.0));
    let coarse = builder.add_tool(ball_tool_config(COARSE_DIAMETER));
    let fine = builder.add_tool(ball_tool_config(FINE_DIAMETER));
    let coarse_id = builder.tools()[coarse].id.0;
    let fine_id = builder.tools()[fine].id.0;
    let model_id = builder.add_model(mesh_model(mesh, "knolls_and_wall"));
    let mut session = builder.build();
    let outcome = session
        .plan_multitool_finishing(&MultitoolPlanSpec {
            model_id,
            tool_ids: vec![coarse_id, fine_id],
            ..MultitoolPlanSpec::default()
        })
        .expect("the ladder plans");
    let fine_tp = *outcome.toolpath_ids.last().expect("two tiers");
    let (index, _) = session.find_toolpath_config_by_id(fine_tp).unwrap();
    let _ = session
        .apply(Command::SetBoundaryConfig(SetBoundaryConfigArgs {
            index,
            boundary: BoundaryConfig::default(),
        }))
        .unwrap();
    let _ = session
        .apply(Command::SetStockSource(SetStockSourceArgs {
            index,
            source: StockSource::Fresh,
        }))
        .unwrap();
    (session, index)
}

fn set_arcs(session: &mut ProjectSession, index: usize, arcs: Option<f64>) {
    let mut dressups = session.get_toolpath_config(index).unwrap().dressups.clone();
    dressups.arc_fitting = arcs.map(|tolerance| ArcFitParams { tolerance });
    let _ = session
        .apply(Command::SetDressupConfig(SetDressupConfigArgs {
            index,
            dressups: Box::new(dressups),
        }))
        .unwrap();
}

/// The low tail of `column_deviations` inside the knoll/wall mask:
/// (min, p0.1, p1, columns).
fn gouge_tail(session: &mut ProjectSession, index: usize) -> (f64, f64, f64, usize) {
    let skip: Vec<_> = session
        .toolpath_configs()
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != index)
        .map(|(_, tc)| tc.id)
        .collect();
    let opts = SimulationOptions {
        resolution: SIM_CELL_MM,
        skip_ids: skip,
        ..Default::default()
    };
    session
        .run_simulation(&opts, &AtomicBool::new(false))
        .expect("the fine tier simulates");
    let sim = session.simulation_result().unwrap();
    let mut devs: Vec<f64> = sim
        .column_deviations
        .as_ref()
        .expect("a reference mesh was supplied")
        .iter()
        .filter(|c| in_mask(c.x, c.y))
        .map(|c| f64::from(c.dev))
        .collect();
    devs.sort_by(f64::total_cmp);
    let at = |q: f64| devs[((devs.len() as f64 - 1.0) * q).round() as usize];
    (devs[0], at(0.001), at(0.01), devs.len())
}

/// The plan's F4 sentry: an arc fit at tolerance `t` buries the tool no
/// deeper than the arcs-off path plus `t`, for every `t` the tier ops can
/// carry (the emitted 0.05 and the candidate cusp/2 = 0.015).
///
/// Runs on the mesa alone ([`sentry_mesh`]). Holds on 2026-09-30, measured:
/// arcs off 0.0513 mm (the source polyline's own sag), 0.05 → 0.0882 mm
/// (450 arcs, the excess all on arc moves on the mesa wall), 0.015 →
/// 0.0513 mm. The 0.05 arm's excess, 0.037 mm, is the residual mechanism;
/// it sits inside this bound, so the bound does not name the defect by
/// itself — `RESULTS.md` F4 does.
#[test]
fn an_arc_fit_buries_no_deeper_than_the_source_path_plus_its_tolerance() {
    let (mut session, index) = session_with_fine_tier(sentry_mesh(), SENTRY_HALF);
    let m = sentry_mesh();
    let idx = SpatialIndex::build_auto(&m);
    let cancel = AtomicBool::new(false);
    let mut run = |arcs: Option<f64>| {
        set_arcs(&mut session, index, arcs);
        session
            .generate_toolpath(index, &cancel)
            .expect("generates");
        let tp = session.get_result(index).unwrap().toolpath().clone();
        let arcs_n = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(
                    m.move_type,
                    MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
                )
            })
            .count();
        (burial(&tp, &m, &idx), arcs_n)
    };
    let (off, off_arcs) = run(None);
    eprintln!("arcs off: burial {:.4} mm", off.max_all);
    assert_eq!(off_arcs, 0, "arcs off emits no arc");
    assert!(
        off.max_all > 0.0,
        "non-vacuity: the source path's own sag reads as burial"
    );
    for t in [0.05, 0.015] {
        let (on, arcs_n) = run(Some(t));
        eprintln!(
            "arcs {t}: {arcs_n} arcs, burial {:.4} mm (arc moves {:.4})",
            on.max_all, on.max_arc
        );
        assert!(arcs_n > 0, "non-vacuity: the mesa wall fits arcs at {t}");
        assert!(
            on.max_all <= off.max_all + t + 1e-6,
            "arcs at {t}: burial {:.4} mm (arc moves {:.4}) exceeds arcs off {:.4} + {t}",
            on.max_all,
            on.max_arc,
            off.max_all
        );
    }
}

#[test]
#[ignore = "generates and simulates three arms of a 40 x 40 mm finish; run with --ignored --nocapture"]
fn arc_fit_burial_against_arcs_off_on_knolls_and_a_wall() {
    let (mut session, index) = session_with_fine_tier(mesh(), HALF);
    let tc = session.get_toolpath_config(index).unwrap().clone();
    eprintln!(
        "fine tier '{}': emitted arcs {:?}, lead-in/out {:?}",
        tc.name, tc.dressups.arc_fitting, tc.dressups.lead_in_out
    );
    let m = mesh();
    let idx = SpatialIndex::build_auto(&m);
    let cancel = AtomicBool::new(false);
    let r = FINE_DIAMETER / 2.0;
    let sag_s05 = r - (r * r - 0.25f64.powi(2)).sqrt();
    eprintln!(
        "knoll sphere r {:.4} mm, CL bump width {:.3} mm, wall {WALL_DEG} deg x {WALL_H} mm, \
         source sag at s 0.5 = {sag_s05:.4} mm",
        knoll_sphere_r(),
        2.0 * (2.0 * r * KNOLL_H - KNOLL_H * KNOLL_H).sqrt()
    );
    eprintln!(
        "{:>6}  {:>6}  {:>5}  {:>8}  {:>8}  {:>8}  {:>6}  {:>6}  {:>6}  {:>8}  {:>8}  {:>8}",
        "arm",
        "moves",
        "arcs",
        "bur max",
        "bur arc",
        "bur lin",
        ">.005",
        ">.015",
        ">.05",
        "dev min",
        "p0.1",
        "p1"
    );
    for (label, arcs) in [
        ("A 0.05", Some(0.05)),
        ("B off", None),
        ("C .015", Some(0.015)),
    ] {
        set_arcs(&mut session, index, arcs);
        session
            .generate_toolpath(index, &cancel)
            .expect("generates");
        let tp = session.get_result(index).unwrap().toolpath().clone();
        let arcs_n = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(
                    m.move_type,
                    MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
                )
            })
            .count();
        let b = burial(&tp, &m, &idx);
        let cutter = build_cutter(&ball_tool_config(FINE_DIAMETER));
        let probe = EntrySurfaceProbe {
            mesh: &m,
            index: &idx,
            cutter: &cutter,
            stock_to_leave: 0.0,
            off_mesh: OffMeshEntry::PlungeFallback,
            rest_stock: None,
        };
        let samples = sample_burial(&tp, &probe);
        let (dmin, p01, p1, n) = gouge_tail(&mut session, index);
        eprintln!(
            "{label:>6}  {:>6}  {arcs_n:>5}  {:>8.4}  {:>8.4}  {:>8.4}  {:>6}  {:>6}  {:>6}  \
             {dmin:>8.4}  {p01:>8.4}  {p1:>8.4}  ({n} mask columns)",
            tp.moves.len(),
            b.max_all,
            b.max_arc,
            b.max_linear,
            b.over_0_005,
            b.over_0_015,
            b.over_0_05,
        );
        print_sample_burial(label, &samples);
    }
}

/// The same A/B/C on the rivmap100 fine tier at tolerance 0.15
/// (`planning/fixtures/rivmap100/rivmap100_tiered_finish.toml`), primary
/// measure only. The floor is the tier's own taper. Stock Fresh, as in the
/// Step 0 instrument: the rest chain needs a simulation first, and a
/// finishing path follows the mesh either way.
#[test]
#[ignore = "generates the rivmap100 fine tier three times (minutes each in debug); run with --ignored --nocapture"]
fn arc_fit_burial_on_the_rivmap100_fine_tier() {
    use rs_cam_core::compute::config::BoundarySource;

    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../planning/fixtures/rivmap100/rivmap100_tiered_finish.toml");
    let mut session = ProjectSession::load(&path).expect("load the rivmap100 tiered project");
    let index = session
        .toolpath_configs()
        .iter()
        .position(|tc| {
            tc.planner_origin.as_ref().is_some_and(|o| o.tier == 1)
                && matches!(
                    tc.boundary.source,
                    BoundarySource::PlannedTierRegions { .. }
                )
        })
        .expect("the fixture carries a planned fine tier");
    let _ = session
        .apply(Command::SetStockSource(SetStockSourceArgs {
            index,
            source: StockSource::Fresh,
        }))
        .unwrap();
    let tc = session.get_toolpath_config(index).unwrap().clone();
    let tool = session
        .tools()
        .iter()
        .find(|t| t.id.0 == tc.tool_id)
        .unwrap()
        .clone();
    let cutter = build_cutter(&tool);
    let mesh = session
        .models()
        .iter()
        .find(|m| m.id == tc.model_id)
        .and_then(|m| m.mesh.clone())
        .expect("the terrain mesh");
    let idx = SpatialIndex::build_auto(&mesh);
    let probe = EntrySurfaceProbe {
        mesh: &mesh,
        index: &idx,
        cutter: &cutter,
        stock_to_leave: 0.0,
        off_mesh: OffMeshEntry::PlungeFallback,
        rest_stock: None,
    };
    let cancel = AtomicBool::new(false);
    eprintln!(
        "rivmap100 '{}' ({}), emitted arcs {:?}",
        tc.name, tool.name, tc.dressups.arc_fitting
    );
    eprintln!(
        "{:>6}  {:>6}  {:>5}  {:>8}  {:>8}  {:>8}  {:>6}  {:>6}  {:>6}",
        "arm", "moves", "arcs", "bur max", "bur arc", "bur lin", ">.005", ">.015", ">.05"
    );
    for (label, arcs) in [
        ("A 0.05", Some(0.05)),
        ("B off", None),
        ("C .015", Some(0.015)),
    ] {
        set_arcs(&mut session, index, arcs);
        session
            .generate_toolpath(index, &cancel)
            .expect("generates");
        let tp = session.get_result(index).unwrap().toolpath().clone();
        let arcs_n = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(
                    m.move_type,
                    MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
                )
            })
            .count();
        let reports = buried_fed_chords(&tp, &probe, SAMPLE_MM, f64::NEG_INFINITY, |_| true);
        let b = summarise(&tp, &reports);
        eprintln!(
            "{label:>6}  {:>6}  {arcs_n:>5}  {:>8.4}  {:>8.4}  {:>8.4}  {:>6}  {:>6}  {:>6}",
            tp.moves.len(),
            b.max_all,
            b.max_arc,
            b.max_linear,
            b.over_0_005,
            b.over_0_015,
            b.over_0_05,
        );
        print_sample_burial(label, &sample_burial(&tp, &probe));
        print_links_and_time(label, &session, &tp);
    }
}
