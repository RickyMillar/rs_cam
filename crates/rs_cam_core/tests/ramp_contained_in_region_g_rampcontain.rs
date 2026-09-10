//! G-RAMPCONTAIN sentry — a ramp entry on a PRISM operation stays inside
//! the operation's own region.
//!
//! UX-R03-001 / F4.1, designed in
//! `planning/ui_fix_2026-09-09/research/R0.2.md`.
//!
//! `dressup::emit_ramp` drew two straight legs of
//! `ENTRY_CLEARANCE / tan(angle) / 2` mm — 19.08 mm at the shipped 3
//! degrees, for EVERY depth per pass — from the entry column along the
//! direction of the first following chord. The emitter received `start`,
//! `end`, `dir`, the angle, the feed and the safety block: no region, no
//! polygon, no tool radius. It could not constrain the legs, and nothing
//! else checked them. On the demo-pocket shape the return leg descended
//! from stock top to full depth while it walked back across stock OUTSIDE
//! the pocket wall.
//!
//! Neither shipped checker could see it. `rapid_collision_count` audits
//! RAPIDS, and these are fed moves. `entry_audit::buried_fed_chords` audits
//! Z — how far a fed move sank below the protected surface — and a leg
//! outside the wall is at a legal Z for its own column. The defect is in
//! XY, so this file measures XY: `entry_audit::fed_moves_outside_region`.
//!
//! ## The fix this pins
//!
//! On the branch that has no surface probe — that is, on a prism operation
//! — the ramp now FOLDS along the operation's own following cut moves
//! instead of drawing blind legs. Every point of that polyline is a
//! tool-centre point the generator itself placed at this level, so the
//! entry is contained wherever the operation's own cut is contained. Below
//! `max(1.0, tool_radius)` mm of following cut it degrades to a plunge at
//! the entry column, which is a cut point of the operation by construction.
//! It never refuses. G-RAMPTERRAIN's Z clip on surface-riding ops is
//! untouched, and so is the adaptive3d door, which passes no fold.
//!
//! ## Why this cannot pass vacuously
//!
//! `fed_moves_outside_region` returns an empty list for an EMPTY region and
//! for a toolpath with no matching move, so "no findings" alone proves
//! nothing. Every arm therefore asserts its POPULATION first: the operation
//! emitted cutting moves, it emitted `EntryRamp` moves (so the fix folded
//! rather than degrading every entry to a plunge), and the checker looked
//! at a non-empty region.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;
use common::make_endmill_6mm;

use std::f64::consts::TAU;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::entry_audit::{fed_moves_outside_region, is_entry_intent};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};

/// Ø6 flat end mill.
const TOOL_RADIUS_MM: f64 = 3.0;
/// Sample spacing for the XY audit, matching `buried_fed_chords`.
const SAMPLE_SPACING_MM: f64 = 0.5;
/// A pocket ring is inset by exactly the tool radius, so the disc touches
/// the wall at zero clearance. This absorbs the arithmetic of that touch.
const OUTSIDE_TOL_MM: f64 = 0.05;
/// Disc radius for a stock probe. It must exceed the simulation cell, or a
/// disc smaller than the grid spacing contains no ray centre, every probe
/// returns `None`, and an all-empty sweep looks exactly like a clean board.
const PROBE_RADIUS_MM: f64 = 0.6;

/// The `fixtures/demo_pocket.svg` shape, built in code.
///
/// R0.2 section 5: the R03 NC places the wall at X = 15 while the SVG
/// draws the rect at x = 5, and the cause of that 10 mm frame shift was
/// not traced. A synthetic polygon in the toolpath's own frame removes the
/// question without changing what is measured.
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
        // CW for holes (negative area)
        hole.push(P2::new(cx + r * (-t).cos(), cy + r * (-t).sin()));
    }
    Polygon2::with_holes(exterior, vec![hole])
}

fn pocket_session(dressups: DressupConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    session.set_stock_config(StockConfig {
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

    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;

    let model_id = session.add_model(LoadedModel {
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

    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(PocketConfig {
            stepover: 2.0,
            depth: 12.0,
            // The R03 depth per pass. The straight legs were 19.08 mm at
            // every DPP, so this value does not set the defect's size —
            // it only sets how many entries there are to check.
            depth_per_pass: 1.2,
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
    session.add_toolpath(0, tc).expect("add pocket toolpath");
    session
}

fn generate(dressups: DressupConfig) -> (ProjectSession, Toolpath) {
    let mut session = pocket_session(dressups);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate pocket toolpath");
    let tp = session
        .get_result(0)
        .expect("pocket toolpath result")
        .toolpath()
        .clone();
    (session, tp)
}

fn count_intent(tp: &Toolpath, intent: MoveIntent) -> usize {
    tp.moves.iter().filter(|m| m.intent == intent).count()
}

fn cutting_move_count(tp: &Toolpath) -> usize {
    tp.moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .count()
}

// ---------------------------------------------------------------------------
// Arm 1 — the acceptance criterion
// ---------------------------------------------------------------------------

#[test]
fn a_ramp_entry_never_reaches_outside_the_pocket() {
    let (_session, tp) = generate(DressupConfig::for_op(OperationType::Pocket));
    let region = vec![demo_pocket_polygon()];

    // Population, before any verdict is read.
    let cuts = cutting_move_count(&tp);
    let ramps = count_intent(&tp, MoveIntent::EntryRamp);
    assert!(cuts > 0, "population: the pocket emitted no cutting moves");
    assert!(
        !region.is_empty(),
        "population: the region is empty, so the checker abstains"
    );
    assert!(
        ramps > 0,
        "population: no EntryRamp moves — the fix must FOLD the ramp, not \
         degrade every entry to a plunge. moves={} cutting={cuts}",
        tp.moves.len()
    );

    let findings = fed_moves_outside_region(
        &tp,
        &region,
        TOOL_RADIUS_MM,
        SAMPLE_SPACING_MM,
        OUTSIDE_TOL_MM,
        is_entry_intent,
    );

    println!(
        "G-RAMPCONTAIN: moves={} cutting={cuts} EntryRamp={ramps} \
         EntryPlunge={} outside-region findings={}",
        tp.moves.len(),
        count_intent(&tp, MoveIntent::EntryPlunge),
        findings.len()
    );
    for f in findings.iter().take(8) {
        println!(
            "  move {} {:?}: disc {:.3} mm outside at ({:.3}, {:.3}, {:.3})",
            f.move_index, f.intent, f.max_outside_mm, f.worst.x, f.worst.y, f.worst.z
        );
    }

    let worst = findings
        .iter()
        .map(|f| f.max_outside_mm)
        .fold(0.0_f64, f64::max);
    assert!(
        findings.is_empty(),
        "{} entry moves reach outside the pocket region; worst disc overhang \
         {worst:.3} mm. A ramp entry must stay inside the operation's own \
         region (G-RAMPCONTAIN).",
        findings.len()
    );
}

// ---------------------------------------------------------------------------
// Arm 2 — the mechanism, not only the outcome
// ---------------------------------------------------------------------------

/// XY distance from `p` to the nearest point of the polyline `path`.
fn xy_distance_to_polyline(p: &P3, path: &[P3]) -> f64 {
    let mut best = f64::INFINITY;
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len_sq = dx * dx + dy * dy;
        let t = if len_sq < 1e-12 {
            0.0
        } else {
            (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0)
        };
        let (qx, qy) = (a.x + dx * t, a.y + dy * t);
        let d = ((p.x - qx).powi(2) + (p.y - qy).powi(2)).sqrt();
        if d < best {
            best = d;
        }
    }
    best
}

/// The fed cut polyline that follows the entry ending at `entry_end`.
fn following_cut(moves: &[Move], entry_end: usize) -> Vec<P3> {
    let mut pts = Vec::new();
    if entry_end == 0 || entry_end > moves.len() {
        return pts;
    }
    pts.push(moves[entry_end - 1].target);
    for m in &moves[entry_end..] {
        if matches!(m.move_type, MoveType::Rapid) || is_entry_intent(m.intent) {
            break;
        }
        pts.push(m.target);
    }
    pts
}

#[test]
fn the_ramp_rides_the_operations_own_cut() {
    // Arc fitting and the rapid reorder are OFF so the emitted fold is the
    // fold, not a re-fitted approximation of it. The containment arm above
    // runs the shipped chain; this one pins the mechanism.
    let shipped = DressupConfig::for_op(OperationType::Pocket);
    let (_session, tp) = generate(DressupConfig {
        arc_fitting: false,
        segment_merge: false,
        optimize_rapid_order: false,
        ..shipped
    });

    // Walk each EntryRamp run and measure it against the cut it leads into.
    let mut checked = 0usize;
    let mut worst = 0.0_f64;
    let mut i = 0usize;
    while i < tp.moves.len() {
        if tp.moves[i].intent != MoveIntent::EntryRamp {
            i += 1;
            continue;
        }
        let start = i;
        while i < tp.moves.len() && tp.moves[i].intent == MoveIntent::EntryRamp {
            i += 1;
        }
        let cut = following_cut(&tp.moves, i);
        if cut.len() < 2 {
            continue;
        }
        for m in &tp.moves[start..i] {
            let d = xy_distance_to_polyline(&m.target, &cut);
            if d > worst {
                worst = d;
            }
        }
        checked += 1;
    }

    println!(
        "G-RAMPCONTAIN mechanism: {checked} entry runs, worst XY offset from the cut {worst:.4} mm"
    );
    assert!(
        checked > 0,
        "population: no EntryRamp run was measured against a following cut"
    );
    assert!(
        worst < 0.05,
        "an EntryRamp point sits {worst:.4} mm off the cut polyline it \
         should ride. The fold walks the operation's own moves, so every \
         ramp point is ON that path by construction; a nonzero offset means \
         the blind legs are back."
    );
}

// ---------------------------------------------------------------------------
// Arm 3 — the simulation half of the F4.1 acceptance
// ---------------------------------------------------------------------------

#[test]
fn checkpoint_zero_removes_nothing_outside_the_pocket_outline() {
    let (mut session, _tp) = generate(DressupConfig::for_op(OperationType::Pocket));
    let cancel = AtomicBool::new(false);
    session
        .run_simulation(
            &SimulationOptions {
                resolution: 0.5,
                skip_ids: Vec::new(),
                metrics_enabled: true,
                auto_resolution: false,
                use_predicted_feed_in_gates: false,
                adaptive_feed_modulation: false,
                modulation_strategy:
                    rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
                modulation_aggressiveness: 1.0,
            },
            &cancel,
        )
        .expect("simulation completes");

    let sim = session.simulation_result().expect("simulation result");
    let checkpoint = sim.checkpoints.first().expect("checkpoint 0");

    // The checkpoint frame is DERIVED, never assumed. Its doc says the
    // stock is zero-rooted (world + 10 here, because the stock origin is
    // (-10, -10)), but a wrong guess would sample the wrong columns and
    // then report a clean board whatever the toolpath did.
    //
    // The ISLAND is what separates the two hypotheses. It stands uncut at
    // stock top; under the wrong offset its probe lands 14 mm away, inside
    // the pocket, where a depth-12 cut through 12 mm of stock leaves no
    // material at all and the probe returns `None`. Three conditions have
    // to hold together: the island reads, the far corner reads, the two
    // agree (both uncut), and a point well inside the pocket is gone.
    let probe = |off: f64, x: f64, y: f64| {
        checkpoint
            .stock
            .max_top_z_in_disc(x + off, y + off, PROBE_RADIUS_MM)
    };
    let mut frame = None;
    for off in [10.0_f64, 0.0] {
        let (Some(island), Some(corner)) = (probe(off, 40.0, 30.0), probe(off, 85.0, 85.0)) else {
            continue;
        };
        if (island - corner).abs() > 0.1 {
            continue;
        }
        if probe(off, 10.0, 10.0).is_some_and(|t| t > corner - 5.0) {
            continue;
        }
        frame = Some((off, corner));
        break;
    }
    let (offset, stock_top_local) = frame.expect(
        "population: no frame hypothesis puts the island uncut, the far \
         corner uncut at the same height, and the pocket interior cut \
         through — so the checkpoint stock was not read",
    );
    println!(
        "G-RAMPCONTAIN sim: frame offset {offset:.1}, uncut top \
         {stock_top_local:.3}, cell {:.3} mm",
        sim.column_grid_cell_mm
    );

    // The outermost ring is inset by EXACTLY the tool radius, so legitimate
    // cutting reaches the wall and stops. The margin is therefore two sim
    // cells of discretisation slack, not a tool radius: a margin wide enough
    // to hide a disc overhang would make this arm pass on the very defect it
    // exists to catch.
    let margin = 1.0;
    let mut cut_outside: Vec<(f64, f64, f64)> = Vec::new();
    let mut read = 0usize;
    let mut missed = 0usize;
    let mut x = -9.0;
    while x < 89.0 {
        let mut y = -9.0;
        while y < 89.0 {
            let inside_band =
                x > 5.0 - margin && x < 75.0 + margin && y > 5.0 - margin && y < 55.0 + margin;
            if !inside_band {
                match probe(offset, x, y) {
                    Some(top) => {
                        read += 1;
                        if top < stock_top_local - 0.1 {
                            cut_outside.push((x, y, top - stock_top_local));
                        }
                    }
                    None => missed += 1,
                }
            }
            y += 1.0;
        }
        x += 1.0;
    }

    println!(
        "G-RAMPCONTAIN sim: {read} columns READ outside the outline ({missed} \
         empty), {} cut, rapid collisions {}",
        cut_outside.len(),
        sim.rapid_collisions.len()
    );
    for (cx, cy, d) in cut_outside.iter().take(8) {
        println!("  column ({cx:.1}, {cy:.1}) lowered {d:.3} mm");
    }

    // Population: the sweep must have actually READ the stock. An all-empty
    // sweep reports zero cut columns and looks identical to a clean board.
    assert!(
        read > missed && read > 100,
        "population: only {read} of {} sampled columns returned a reading, so \
         the sweep did not measure the stock",
        read + missed
    );
    assert!(
        cut_outside.is_empty(),
        "{} stock columns outside the 70x50 pocket outline were cut; the \
         deepest is {:.3} mm. The pocket must remove nothing out there.",
        cut_outside.len(),
        cut_outside
            .iter()
            .map(|c| c.2)
            .fold(0.0_f64, |a, b| if b < a { b } else { a })
    );
}
