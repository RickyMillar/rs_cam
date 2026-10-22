//! Geometry selection P6.6 (design 4.4, lead rulings L-11, L-12, L-15):
//! a Bore mills each circle to its drawn size with a helix.
//!
//! # The claims
//!
//! 1. On the simulated stock (cell [`CELL_MM`]) a 6 mm end mill bores a
//!    10 mm and a 12 mm circle. Along each of eight rays from the centre
//!    the wall stands at `d / 2` within one cell. Every cell more than one
//!    cell inside the wall is at the floor Z within [`FLOOR_EPS_MM`]: the
//!    floor is flat and the hole has no core (`d <= 2 D`). With the floor
//!    at the stock bottom the hole goes through: every such column is
//!    empty.
//! 2. Under the default rule a Bore leaves out the circle of the tool
//!    size, the smaller circle and the circle of more than two tool
//!    diameters, with one note each that names the fix. It does not cut
//!    the point (a point states no size) and gives no note for it.
//! 3. A pick of the circle that leaves a core (`LeavesCore`), of the
//!    smaller circle (`Smaller`) or of the tool-size circle refuses
//!    `HoleDoesNotFit`, by item, with the fix text.
//! 4. On the remaining stock (`RestUse::StockColumn`, D8) a Bore starts
//!    each helix at the column top over its circle plus the start
//!    clearance: under a prior pocket 2 mm deep, at Z -1.5; on the uncut
//!    stock, at Z 0.5.
//! 5. The helix and the floor turns are cut intents, never `EntryHelix`
//!    (L-15).
//!
//! Red proof (run, red, reverted): the kernel with the helix at `d / 2`
//! (no tool offset). Claim 1 went red: "d 10: the wall stands at 8.150 mm,
//! not d/2 = 5 mm within one cell". The eight rays read 8.000 to 8.150 mm,
//! `D / 2` = 3 mm past `d / 2`.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_bore_cuts_the_stated_size_p6_6
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The measured wall radius and floor of each hole ARE the record.
#![allow(clippy::print_stderr)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::session::{
    TestHole, drawing_items, polygon_model, single_op_session_with, stock_under, toolpath_config,
};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::{BoreConfig, PocketConfig};
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::selection::hole_fit::DrillSize;
use rs_cam_core::selection::{
    HoleFit, HoleFitRule, MatchKey, NoteCause, RefusalCause, Selection, SelectionEdit, SourceRef,
};
use rs_cam_core::session::{
    Command, EditSelectionArgs, LoadedModel, ProjectSession, ProjectSessionBuilder,
    SelectionTarget, SessionError, SetSimulationResolutionArgs, SimulationOptions,
    SimulationResolution,
};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

/// The tool diameter `D` (mm).
const TOOL_D: f64 = 6.0;
/// The simulation cell (mm).
const CELL_MM: f64 = 0.25;
/// The float noise (mm) of a floor cell: the dexel Z is an `f32`.
const FLOOR_EPS_MM: f64 = 1e-3;
/// The floor depth below the stock top (mm).
const DEPTH: f64 = 4.0;
/// The stock half extent and height (mm). The top is at Z 0.
const STOCK_HALF: f64 = 30.0;
const STOCK_HEIGHT: f64 = 10.0;

/// The two circles a 6 mm tool bores: `D < d <= 2 D`.
const BORED: [([f64; 2], f64); 2] = [([-12.0, 0.0], 10.0), ([12.0, 0.0], 12.0)];
/// The circles a Bore leaves out (or refuses when picked).
const TOOL_SIZE_AT: [f64; 2] = [0.0, 15.0];
const SMALLER_AT: [f64; 2] = [-20.0, 15.0];
const CORE_AT: [f64; 2] = [20.0, 17.0];
const CORE_D: f64 = 14.0;
const POINT_AT: [f64; 2] = [0.0, -15.0];

fn hole(xy: [f64; 2], diameter: Option<f64>) -> TestHole {
    TestHole {
        x: xy[0],
        y: xy[1],
        layer: "HOLES".to_owned(),
        diameter,
    }
}

/// The two bored circles, the three misfit circles and a point.
fn hole_model() -> LoadedModel {
    let mut holes: Vec<TestHole> = BORED.iter().map(|&(xy, d)| hole(xy, Some(d))).collect();
    holes.extend([
        hole(TOOL_SIZE_AT, Some(TOOL_D)),
        hole(SMALLER_AT, Some(4.0)),
        hole(CORE_AT, Some(CORE_D)),
        hole(POINT_AT, None),
    ]);
    let mut model = polygon_model(Vec::new(), "p6_6_holes");
    model.items = Arc::new(drawing_items(Vec::new(), &holes));
    model
}

fn bore(floor: Floor) -> OperationConfig {
    OperationConfig::Bore(BoreConfig {
        floor,
        ..BoreConfig::default()
    })
}

fn session(op: OperationConfig) -> ProjectSession {
    single_op_session_with(
        stock_under(STOCK_HALF, STOCK_HEIGHT),
        endmill_tool_config(TOOL_D),
        hole_model(),
        "p6_6",
        op,
        |_| {},
    )
}

fn generate(s: &mut ProjectSession, index: usize) -> Result<Toolpath, SessionError> {
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(index, &cancel)
        .map(|r| r.op_data.annotated().toolpath.clone())
}

/// Simulate every toolpath at [`CELL_MM`] and give the final stock.
fn simulate(s: &mut ProjectSession) -> Arc<TriDexelStock> {
    let _ = s
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .unwrap();
    let cancel = AtomicBool::new(false);
    let _ = s
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(CELL_MM),
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .unwrap();
    s.simulation_result()
        .and_then(|sim| sim.checkpoints.last().map(|c| Arc::clone(&c.mesh_stock)))
        .expect("a checkpoint")
}

/// The material top (mm) of the cell nearest `(x, y)`; `None` when the
/// column is empty.
fn top_at(stock: &TriDexelStock, x: f64, y: f64) -> Option<f64> {
    let g = &stock.z_grid;
    let col = ((x - g.origin_u) / g.cell_size).round() as usize;
    let row = ((y - g.origin_v) / g.cell_size).round() as usize;
    g.top_z_at(row, col).map(f64::from)
}

/// The radius of the wall along the ray at `angle` from `centre`: the
/// first radius (sampled at a fifth of a cell) where the material stands
/// above the mid height between the floor and the stock top.
fn wall_radius(stock: &TriDexelStock, centre: [f64; 2], angle: f64, floor_z: f64) -> f64 {
    let mid = floor_z / 2.0;
    let step = CELL_MM / 5.0;
    (0..)
        .map(|k| f64::from(k) * step)
        .take_while(|r| *r < 20.0)
        .find(|r| {
            let (x, y) = (centre[0] + r * angle.cos(), centre[1] + r * angle.sin());
            top_at(stock, x, y).is_some_and(|t| t > mid)
        })
        .expect("a wall within 20 mm")
}

/// Every cell centre within `radius` of `centre`.
fn cells_within(stock: &TriDexelStock, centre: [f64; 2], radius: f64) -> Vec<(f64, f64)> {
    let g = &stock.z_grid;
    let mut out = Vec::new();
    for row in 0..g.rows {
        for col in 0..g.cols {
            let x = g.origin_u + col as f64 * g.cell_size;
            let y = g.origin_v + row as f64 * g.cell_size;
            if (x - centre[0]).hypot(y - centre[1]) < radius {
                out.push((x, y));
            }
        }
    }
    out
}

/// Claim 1: the wall at `d / 2`, the floor flat, no core.
#[test]
fn a_bore_cuts_the_wall_at_the_stated_size_and_a_flat_floor() {
    let mut s = session(bore(Floor::top(DEPTH)));
    let tp = generate(&mut s, 0).expect("the bore generates");
    // Claim 5.
    assert!(tp.moves.iter().all(|m| m.intent != MoveIntent::EntryHelix));
    assert!(
        tp.moves
            .iter()
            .any(|m| m.intent == MoveIntent::ClearingCut && m.move_type != MoveType::Rapid)
    );
    let stock = simulate(&mut s);
    let floor_z = -DEPTH;
    for (centre, d) in BORED {
        let walls: Vec<f64> = (0..8)
            .map(|k| {
                wall_radius(
                    &stock,
                    centre,
                    f64::from(k) * std::f64::consts::FRAC_PI_4,
                    floor_z,
                )
            })
            .collect();
        eprintln!(
            "d {d}: wall radius along 8 rays {walls:.3?} (d/2 = {})",
            d / 2.0
        );
        for w in &walls {
            assert!(
                (w - d / 2.0).abs() <= CELL_MM,
                "d {d}: the wall stands at {w:.3} mm, not d/2 = {} mm within one cell",
                d / 2.0
            );
        }
        let inside = cells_within(&stock, centre, d / 2.0 - CELL_MM);
        assert!(!inside.is_empty());
        let worst = inside
            .iter()
            .map(|&(x, y)| (top_at(&stock, x, y).unwrap_or(f64::NEG_INFINITY) - floor_z).abs())
            .fold(0.0_f64, f64::max);
        eprintln!(
            "d {d}: {} floor cells, the worst is {worst:.4} mm off the floor Z {floor_z}",
            inside.len()
        );
        assert!(
            worst <= FLOOR_EPS_MM,
            "d {d}: a cell inside the wall is {worst} mm off the floor (a core or a step)"
        );
    }
}

/// Claim 1, the through hole: the floor at the stock bottom (0.5 mm into
/// the spoilboard) empties every column inside the wall.
#[test]
fn a_bore_to_the_stock_bottom_goes_through() {
    let mut s = session(bore(Floor::StockBottom { depth: 0.5 }));
    let _ = generate(&mut s, 0).expect("the bore generates");
    let stock = simulate(&mut s);
    for (centre, d) in BORED {
        let inside = cells_within(&stock, centre, d / 2.0 - CELL_MM);
        let standing: Vec<_> = inside
            .iter()
            .filter(|&&(x, y)| top_at(&stock, x, y).is_some())
            .collect();
        eprintln!(
            "d {d}: {} of {} columns inside the wall hold material",
            standing.len(),
            inside.len()
        );
        assert!(standing.is_empty(), "d {d}: the hole does not go through");
    }
}

/// The pick key of the item at `xy`.
fn pick_at(s: &ProjectSession, xy: [f64; 2]) -> MatchKey {
    let item = s.models()[0]
        .items
        .iter()
        .find(|i| i.key().anchor == xy)
        .unwrap_or_else(|| panic!("no item at {xy:?}"));
    MatchKey {
        source: SourceRef::ToolpathModel,
        ..item.key().clone()
    }
}

/// Claim 2: the default rule bores the two circles and leaves out the
/// three misfit circles, with one note each. The point gives no note.
#[test]
fn the_default_rule_leaves_out_each_misfit_circle_with_a_note() {
    let mut s = session(bore(Floor::top(DEPTH)));
    let tp = generate(&mut s, 0).expect("the bore generates");
    // The cut stays inside the two bored circles.
    for m in tp.moves.iter().filter(|m| m.move_type != MoveType::Rapid) {
        let near = BORED.iter().any(|&(c, d)| {
            (m.target.x - c[0]).hypot(m.target.y - c[1]) <= (d - TOOL_D) / 2.0 + 1e-9
        });
        assert!(near, "a fed move off the bored circles: {:?}", m.target);
    }
    let finding = s.result_at(0).unwrap().stats.selection_note.clone();
    let notes = finding
        .as_ref()
        .map(|f| f.notes.clone())
        .unwrap_or_default();
    let mut fits: Vec<(HoleFit, [f64; 2])> = notes
        .iter()
        .filter_map(|n| match n.cause {
            NoteCause::HoleFit { fit, size } => {
                assert_eq!(size, DrillSize::strict(TOOL_D));
                Some((fit, n.first.key.anchor))
            }
            NoteCause::KindNotServed | NoteCause::AtTheTop { .. } => None,
        })
        .collect();
    fits.sort_by(|a, b| a.1[0].total_cmp(&b.1[0]));
    assert_eq!(
        fits,
        vec![
            (HoleFit::Smaller, SMALLER_AT),
            (HoleFit::ToolSize, TOOL_SIZE_AT),
            (HoleFit::LeavesCore, CORE_AT),
        ]
    );
    assert_eq!(notes.len(), 3, "the point gives no note: {notes:?}");
    let text = finding.unwrap().message();
    eprintln!("{text}");
    for fix in [
        "drill it with a Drill op",
        "use a smaller tool, or a Drill with a tool of the circle size",
        "use a Pocket on the circle or a larger tool",
    ] {
        assert!(
            text.contains(fix),
            "the note text has no fix '{fix}': {text}"
        );
    }
}

/// Claim 3: a picked misfit circle refuses by item.
#[test]
fn a_picked_misfit_circle_refuses_by_item() {
    for (at, fit, fix) in [
        (
            CORE_AT,
            HoleFit::LeavesCore,
            "Use a Pocket on the circle or a larger tool",
        ),
        (SMALLER_AT, HoleFit::Smaller, "Use a smaller tool"),
        (TOOL_SIZE_AT, HoleFit::ToolSize, "Drill it with a Drill op"),
    ] {
        let mut s = session(bore(Floor::top(DEPTH)));
        let picks = vec![pick_at(&s, BORED[0].0), pick_at(&s, at)];
        let _ = s
            .apply(Command::EditSelection(EditSelectionArgs {
                index: 0,
                target: SelectionTarget::Op,
                edit: SelectionEdit::Replace(Box::new(Selection::from_picks(picks))),
            }))
            .unwrap();
        let err = generate(&mut s, 0).expect_err("the pick refuses");
        let SessionError::Selection(refusal) = err else {
            panic!("not a selection refusal: {err}");
        };
        assert_eq!(
            refusal.cause,
            RefusalCause::HoleDoesNotFit {
                fit,
                size: DrillSize::strict(TOOL_D),
                rule: HoleFitRule::LargerThanTool,
            }
        );
        assert_eq!(refusal.item.as_ref().unwrap().key.anchor, at);
        let text = refusal.to_string();
        eprintln!("{text}");
        assert!(text.starts_with("Bore: circle on layer 'HOLES'"), "{text}");
        assert!(text.contains(fix), "{text}");
    }
}

/// Claim 4: on the remaining stock the helix starts at the column top over
/// the circle plus the start clearance.
#[test]
fn a_bore_on_remaining_stock_starts_at_the_column_top() {
    const PRIOR_DEPTH: f64 = 2.0;
    let mut builder = ProjectSessionBuilder::new().stock(stock_under(STOCK_HALF, STOCK_HEIGHT));
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_D));
    let tool_id = builder.tools()[tool_idx].id.raw();
    // The prior pocket covers the 10 mm circle and the column reach (the
    // hole radius plus a cell) round it, and not the 12 mm circle.
    let [cx, cy] = BORED[0].0;
    let pocket_model = builder.add_model(polygon_model(
        vec![common::op_fixtures::rectangle(
            cx - 8.0,
            cy - 8.0,
            cx + 8.0,
            cy + 8.0,
        )],
        "p6_6_prior",
    ));
    let holes = builder.add_model(hole_model());
    let prior = OperationConfig::Pocket(PocketConfig {
        floor: Floor::top(PRIOR_DEPTH),
        ..PocketConfig::default()
    });
    let _ = builder
        .add_toolpath(
            0,
            toolpath_config(
                "prior",
                prior,
                rs_cam_core::ids::ToolId::from_raw(tool_id),
                pocket_model,
            ),
        )
        .unwrap();
    let mut cfg = toolpath_config(
        "bore",
        bore(Floor::top(DEPTH)),
        rs_cam_core::ids::ToolId::from_raw(tool_id),
        holes,
    );
    cfg.stock_source = StockSource::FromRemainingStock;
    let _ = builder.add_toolpath(0, cfg).unwrap();
    let mut s = builder.build();
    let _ = generate(&mut s, 0).expect("the prior generates");
    let _ = simulate(&mut s);
    let tp = generate(&mut s, 1).expect("the bore generates");
    let clearance = rs_cam_core::dressup::ENTRY_CONTACT_CLEARANCE;
    for (centre, top) in [(BORED[0].0, -PRIOR_DEPTH), (BORED[1].0, 0.0)] {
        // The lowest rapid inside the circle stops at the start Z: the
        // column top over the circle plus the start clearance. (The session
        // stages turn the feed out in the air into a rapid, so the rapid
        // ends at the helix start, not over the centre.)
        let start_z = top + clearance;
        let rapid_z = tp
            .moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid)
            .map(|m| m.target)
            .filter(|p| (p.x - centre[0]).hypot(p.y - centre[1]) < 4.0)
            .map(|p| p.z)
            .fold(f64::INFINITY, f64::min);
        eprintln!("circle at {centre:?}: the rapid stops at Z {rapid_z:.4} (start Z {start_z})");
        assert!(
            (rapid_z - start_z).abs() < 1e-9,
            "circle at {centre:?}: the helix starts at Z {rapid_z}, not at {start_z}"
        );
    }
}
