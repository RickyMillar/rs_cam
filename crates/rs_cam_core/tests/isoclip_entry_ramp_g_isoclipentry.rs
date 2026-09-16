//! G-ISOCLIPENTRY — a clipped re-entry on a REST-DRIVEN surface-riding pass
//! ramps into the standing material instead of plunging into it.
//!
//! # The defect
//!
//! `boundary::clip_toolpath_to_boundary_set_with_provenance` re-enters a
//! region by rapiding to the crossing move's own XY at `safe_z` and feeding
//! straight down to its cut Z. `dressup::optimize_entry_descents` then lowers
//! the rapid to just above the input stock's material ceiling, so the FED part
//! of that descent is exactly the material standing over the entry point.
//!
//! On a fresh-stock pass that is nothing: the ceiling is the model and the
//! descent arrives in air. On a `FromRemainingStock` pass the islands the
//! boundary confines the tool to are, by construction, where the upstream tool
//! could NOT reach — so the fed part is a full-diameter vertical bite into
//! rest material, once per clipped ring re-entry.
//!
//! Measured on the wanaka200 board (`planning/deep_doc_modulation_2026-09-08/`,
//! tier 1 of a two-tier multitool plan clipped to its island regions):
//! `project.entry_load` CRITICAL, peak entry bite 1.67 mm against a 0.13 mm
//! median body bite, over 7 000 entry samples past the bar.
//!
//! G-RAMPTERRAIN (2026-09-03) made the DRESSUP entry moves stock aware, and it
//! cannot reach this: those moves are built before the boundary clip, and the
//! clip rapids them away and invents a descent no door ever sees.
//!
//! # The fix under test
//!
//! `dressup::optimize_entry_descents*` takes a `RestEntryRamp`, `Some` only on
//! a rest-driven surface-riding operation. With it the fed descent becomes
//! bite-budgeted zig-zag laps along the run's own first millimetre —
//! `pencil::plan_entry_ramp` with `end_at_start`, the SAME construction site
//! and the same physical model the pencil family already used against
//! G-ENTRYLOAD.
//!
//! # Arms
//!
//! * `a_clipped_reentry_ramps_into_rest_material` — green. No fed `EntryPlunge`
//!   descends into material, laps exist, and no lap removes more than twice
//!   the bite budget.
//! * `b_without_the_ramp_the_reentry_plunges` — RED, permanently. The same
//!   fixture with `ramp: None` reproduces the full-depth vertical bite, so the
//!   green arm cannot pass by measuring nothing.
//! * `c_the_ramp_hands_the_tool_back_where_the_plunge_would_have` — the
//!   `end_at_start` contract the no-drop provenance rule depends on.
//! * `e_the_dressup_ramp_is_bite_budgeted_over_rest` — the SECOND emitter of
//!   the same defect, found when the fix above moved the wanaka peak by
//!   nothing: with `entry_style = ramp` the generator's plunge never reaches
//!   the post-clip door, because `dressup::apply_entry` has already replaced it
//!   with `emit_ramp`'s two-leg zigzag — which has no bite budget and whose
//!   closing leg takes the whole remaining depth off the start column.
//! * `f_without_the_rest_stock_the_dressup_ramp_takes_the_lot` — the red arm
//!   for that one.
//! * `d_nothing_standing_leaves_the_emission_untouched` — parity for the
//!   PLAN's own abstention: with the entry already on the finished surface the
//!   depth is inside one bite budget, so no lap is emitted and the move list is
//!   the pre-fix one. It does NOT pin the fresh-stock guarantee — that one is
//!   the call site handing `None`, because `gen_initial_stock` is `None` on a
//!   `StockSource::Fresh` operation (`session/compute.rs`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::execute::apply_dressups;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::dressup::{
    EntrySurfaceProbe, OffMeshEntry, RestEntryRamp, optimize_entry_descents,
};
use rs_cam_core::geo::P3;
use rs_cam_core::geometry::boundary::clip_toolpath_to_boundary_set_with_provenance;
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::pencil::{entry_bite_budget_mm, tip_contact_radius};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::trace::transform_provenance::ReconcileSet;

// ── Fixture ─────────────────────────────────────────────────────────────

const CELL_MM: f64 = 0.2;
const STOCK_BOTTOM_Z: f64 = -5.0;
/// The raw stock top, and the top of the rest band standing inside the island.
const STOCK_TOP_Z: f64 = 1.4;
/// What the upstream pass cut the ground down to everywhere it could reach.
const FLOOR_Z: f64 = 0.0;
/// The depth of standing material over every entry point inside the island.
const REST_DEPTH_MM: f64 = STOCK_TOP_Z - FLOOR_Z;

const SAFE_Z: f64 = 12.0;
const CUT_FEED: f64 = 782.0;
const PLUNGE_RATE: f64 = 171.0;

/// The island the tier boundary confines the pass to.
const ISLAND_MIN: f64 = 8.0;
const ISLAND_MAX: f64 = 16.0;
/// Where the clipped path re-enters the island.
const ENTRY_X: f64 = 9.0;
const ENTRY_Y: f64 = 12.0;

/// Geometric slack: one cell, plus the half-cell dilation
/// `max_conservative_top_z_in_disc` adds on purpose.
const TOL_MM: f64 = 0.05;

/// The shipped R1.0 tapered ball from the operator's board — the tier-1 tool
/// in the measured case. `radius()` is the SHANK; the tip that nestles into
/// the surface is `tip_contact_radius`, which is what the planner reads.
fn tapered_ball() -> TaperedBallEndmill {
    TaperedBallEndmill::new(2.0, 7.1, 6.0, 20.0)
}

fn island() -> Polygon2 {
    Polygon2::rectangle(ISLAND_MIN, ISLAND_MIN, ISLAND_MAX, ISLAND_MAX)
}

/// Ground cut to [`FLOOR_Z`] everywhere the upstream tool reached, with the
/// island still standing at the raw stock top.
fn rest_stock(island_stands: bool) -> TriDexelStock {
    let mut stock =
        TriDexelStock::from_stock(0.0, 0.0, 24.0, 24.0, STOCK_BOTTOM_Z, STOCK_TOP_Z, CELL_MM);
    let (rows, cols) = (stock.z_grid.rows, stock.z_grid.cols);
    let (cs, ou, ov) = (
        stock.z_grid.cell_size,
        stock.z_grid.origin_u,
        stock.z_grid.origin_v,
    );
    for row in 0..rows {
        let y = ov + row as f64 * cs;
        for col in 0..cols {
            let x = ou + col as f64 * cs;
            let inside = island_stands
                && (ISLAND_MIN..=ISLAND_MAX).contains(&x)
                && (ISLAND_MIN..=ISLAND_MAX).contains(&y);
            let top = if inside { STOCK_TOP_Z } else { FLOOR_Z };
            stock.clear_above_at(row, col, top as f32);
        }
    }
    stock
}

/// A finishing run that starts outside the island and continues inside it —
/// the shape the ring generator plus the region clip produce. Every cutting
/// point rides the finished surface at [`FLOOR_Z`].
fn crossing_run() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(5.0, ENTRY_Y, SAFE_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(
        P3::new(5.0, ENTRY_Y, FLOOR_Z),
        CUT_FEED,
        MoveIntent::FinishingCut,
    );
    // Outside the island, walking toward it.
    tp.feed_to_with_intent(
        P3::new(7.0, ENTRY_Y, FLOOR_Z),
        CUT_FEED,
        MoveIntent::FinishingCut,
    );
    // The crossing move: the clip turns this one into rapid + descent.
    let mut x = ENTRY_X;
    while x <= 14.0 {
        tp.feed_to_with_intent(
            P3::new(x, ENTRY_Y, FLOOR_Z),
            CUT_FEED,
            MoveIntent::FinishingCut,
        );
        x += 0.25;
    }
    tp
}

/// Clip to the island, then run the entry-descent pass with or without the
/// rest-entry ramp.
fn clipped_and_optimised(stock: &TriDexelStock, ramp: bool) -> Toolpath {
    let cutter = tapered_ball();
    let (mut tp, _mapping) = clip_toolpath_to_boundary_set_with_provenance(
        &crossing_run(),
        std::slice::from_ref(&island()),
        SAFE_Z,
        Some(PLUNGE_RATE),
    );
    let cfg = RestEntryRamp {
        contact_radius_mm: tip_contact_radius(&cutter),
        feed_rate: CUT_FEED,
        plunge_rate: PLUNGE_RATE,
    };
    optimize_entry_descents(
        &mut tp,
        Some(stock),
        STOCK_TOP_Z,
        cutter.envelope_radius_mm(),
        &cutter,
        ramp.then_some(&cfg),
    );
    tp
}

// ── Measures ────────────────────────────────────────────────────────────

/// How far a fed move descends BELOW the material ceiling over its own end
/// point — the vertical bite the simulator reports as `plunge_descent_mm`.
///
/// Reads the same conservative disc primitive the pass itself reads, so the
/// two sides of the comparison are one measure.
fn fed_bite_mm(stock: &TriDexelStock, contact_radius: f64, from: P3, to: P3) -> f64 {
    let Some(ceiling) = stock.max_conservative_top_z_in_disc(to.x, to.y, contact_radius) else {
        return 0.0;
    };
    (from.z.min(ceiling) - to.z).max(0.0)
}

/// The deepest vertical bite any fed `EntryPlunge` move takes.
fn deepest_plunge_bite(tp: &Toolpath, stock: &TriDexelStock, contact_radius: f64) -> f64 {
    let mut worst = 0.0_f64;
    for pair in tp.moves.windows(2) {
        let (prev, cur) = (&pair[0], &pair[1]);
        if cur.intent != MoveIntent::EntryPlunge {
            continue;
        }
        if !matches!(cur.move_type, MoveType::Linear { .. }) {
            continue;
        }
        worst = worst.max(fed_bite_mm(stock, contact_radius, prev.target, cur.target));
    }
    worst
}

fn count_intent(tp: &Toolpath, intent: MoveIntent) -> usize {
    tp.moves.iter().filter(|m| m.intent == intent).count()
}

/// The deepest bite any single ENTRY move takes off one column.
///
/// Seeds every column the entry visits with the stock's own conservative
/// ceiling, then walks the entry's Z values at that column from the top down:
/// the first move to land below the ceiling removes `ceiling - z`, and each
/// later one removes what the previous one left. This is the quantity the
/// per-lap bite budget bounds, and it reads a budgeted lap ladder and an
/// unbudgeted two-leg zigzag on the same scale.
fn worst_entry_step_mm(tp: &Toolpath, stock: &TriDexelStock, contact_radius: f64) -> f64 {
    let mut columns: std::collections::BTreeMap<(i64, i64), Vec<f64>> = Default::default();
    for m in &tp.moves {
        if !matches!(
            m.intent,
            MoveIntent::EntryRamp | MoveIntent::EntryPlunge | MoveIntent::EntryHelix
        ) {
            continue;
        }
        if !matches!(m.move_type, MoveType::Linear { .. }) {
            continue;
        }
        let key = (
            (m.target.x * 1000.0).round() as i64,
            (m.target.y * 1000.0).round() as i64,
        );
        columns.entry(key).or_default().push(m.target.z);
    }
    let mut worst = 0.0_f64;
    for (key, zs) in &mut columns {
        let (x, y) = (key.0 as f64 / 1000.0, key.1 as f64 / 1000.0);
        let Some(ceiling) = stock.max_conservative_top_z_in_disc(x, y, contact_radius) else {
            continue;
        };
        let mut ladder = vec![ceiling];
        zs.sort_by(|a, b| b.total_cmp(a));
        ladder.extend(zs.iter().copied().filter(|z| *z < ceiling));
        for pair in ladder.windows(2) {
            worst = worst.max(pair[0] - pair[1]);
        }
    }
    worst
}

/// Sample spacing (mm) along an entry chord, and the width of the column
/// bucket the samples are grouped into.
const CHORD_SAMPLE_MM: f64 = 0.25;

/// The deepest bite any ENTRY move takes ANYWHERE along its own chord.
///
/// [`worst_entry_step_mm`] reads move TARGETS, which is the right measure for a
/// lap ladder — every lap point IS an endpoint. It says nothing about a long
/// straight leg that dives under standing material in the middle and comes back
/// out, which is exactly what the legacy zigzag does; on a flat model nothing
/// lifts the leg, so the emitter writes two endpoints and the target-only
/// measure reads zero.
///
/// Each entry chord is sampled every [`CHORD_SAMPLE_MM`] and the samples are
/// bucketed by column in EMISSION order. A column's surface starts at the
/// stock's own conservative ceiling and drops to each sample that lands under
/// it, so a lap ladder reads its per-lap step and a single unbudgeted leg reads
/// the whole depth. This is the quantity the simulator reports as
/// `axial_engagement_mm`.
fn worst_entry_chord_bite_mm(tp: &Toolpath, stock: &TriDexelStock, contact_radius: f64) -> f64 {
    let mut surface: std::collections::BTreeMap<(i64, i64), f64> = Default::default();
    let mut worst = 0.0_f64;
    for pair in tp.moves.windows(2) {
        let (prev, cur) = (&pair[0], &pair[1]);
        if !matches!(
            cur.intent,
            MoveIntent::EntryRamp | MoveIntent::EntryPlunge | MoveIntent::EntryHelix
        ) {
            continue;
        }
        if !matches!(cur.move_type, MoveType::Linear { .. }) {
            continue;
        }
        let (a, b) = (prev.target, cur.target);
        let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
        let steps = ((len / CHORD_SAMPLE_MM).ceil() as usize).max(1);
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            let x = a.x + (b.x - a.x) * t;
            let y = a.y + (b.y - a.y) * t;
            let z = a.z + (b.z - a.z) * t;
            let Some(ceiling) = stock.max_conservative_top_z_in_disc(x, y, contact_radius) else {
                continue;
            };
            let key = (
                (x / CHORD_SAMPLE_MM).round() as i64,
                (y / CHORD_SAMPLE_MM).round() as i64,
            );
            let top = surface.entry(key).or_insert(ceiling);
            worst = worst.max(*top - z);
            *top = top.min(z);
        }
    }
    worst
}

// ── Arms ────────────────────────────────────────────────────────────────

#[test]
fn a_clipped_reentry_ramps_into_rest_material() {
    let stock = rest_stock(true);
    let cutter = tapered_ball();
    let contact = tip_contact_radius(&cutter);
    let tp = clipped_and_optimised(&stock, true);

    let ramps = count_intent(&tp, MoveIntent::EntryRamp);
    assert!(
        ramps > 0,
        "the clipped re-entry emitted no ramp at all: {:?}",
        tp.moves
    );

    let bite = deepest_plunge_bite(&tp, &stock, contact);
    assert!(
        bite <= TOL_MM,
        "a fed EntryPlunge still bit {bite:.3} mm into rest material \
         (tolerance {TOL_MM:.3} mm)"
    );

    // No lap may remove more than twice the per-lap budget: two consecutive
    // laps run in opposite directions, so their vertical gap is widest at the
    // turn (`plan_entry_ramp`'s own bound).
    let budget = entry_bite_budget_mm(contact);
    let step = worst_entry_step_mm(&tp, &stock, contact);
    assert!(
        step <= 2.0 * budget + TOL_MM,
        "an entry move took {step:.3} mm off one column, past the {:.3} mm bound",
        2.0 * budget
    );
}

#[test]
fn b_without_the_ramp_the_reentry_plunges() {
    // RED ARM, kept green by asserting the DEFECT. It re-proves each run that
    // the measure above can see a full-depth bite — a green arm that measures
    // nothing is the failure mode this file exists to avoid.
    let stock = rest_stock(true);
    let cutter = tapered_ball();
    let contact = tip_contact_radius(&cutter);
    let tp = clipped_and_optimised(&stock, false);

    assert_eq!(
        count_intent(&tp, MoveIntent::EntryRamp),
        0,
        "no ramp is expected without the door"
    );
    let bite = deepest_plunge_bite(&tp, &stock, contact);
    assert!(
        bite >= REST_DEPTH_MM - TOL_MM,
        "pre-fix reproduction lost: the plunge bit only {bite:.3} mm of the \
         {REST_DEPTH_MM:.3} mm standing over the entry"
    );
}

#[test]
fn c_the_ramp_hands_the_tool_back_where_the_plunge_would_have() {
    // `end_at_start`: the laps end on the run's FIRST point at its finished Z,
    // which is the plunge's own target. The body pass that follows is
    // untouched, which is what lets the pass insert the manoeuvre under a
    // provenance contract in which no input move may be dropped.
    let stock = rest_stock(true);
    let tp = clipped_and_optimised(&stock, true);

    let last_ramp = tp
        .moves
        .iter()
        .rposition(|m| m.intent == MoveIntent::EntryRamp)
        .expect("no ramp emitted");
    let end = tp.moves[last_ramp].target;
    assert!(
        (end.x - ENTRY_X).abs() < 1e-6 && (end.y - ENTRY_Y).abs() < 1e-6,
        "the ramp ended at ({:.3}, {:.3}), not the re-entry point",
        end.x,
        end.y
    );
    assert!(
        (end.z - FLOOR_Z).abs() < 1e-6,
        "the ramp ended at Z {:.4}, not the finished surface",
        end.z
    );
    // The body pass resumes immediately, at the finished Z.
    let next = &tp.moves[last_ramp + 1];
    assert_eq!(next.intent, MoveIntent::FinishingCut);
    assert!((next.target.z - FLOOR_Z).abs() < 1e-6);
}

#[test]
fn d_nothing_standing_leaves_the_emission_untouched() {
    // Parity for the PLAN's abstention, not for the fresh-stock gate. With
    // nothing standing over the entry the depth is inside one bite budget, so
    // the door costs no motion where it buys nothing. The fresh-stock case is
    // pinned at the call site instead: `gen_initial_stock` is `None` there, so
    // `ramp` is never consulted.
    let stock = rest_stock(false);
    let with_ramp = clipped_and_optimised(&stock, true);
    let without = clipped_and_optimised(&stock, false);

    assert_eq!(
        with_ramp.moves.len(),
        without.moves.len(),
        "the door changed a descent that had no material under it"
    );
    for (a, b) in with_ramp.moves.iter().zip(without.moves.iter()) {
        assert_eq!(a.intent, b.intent);
        assert_eq!(a.move_type, b.move_type);
        assert!((a.target.z - b.target.z).abs() < 1e-12);
    }
}

// ── The dressup ramp (`emit_ramp`) ──────────────────────────────────────

/// A flat model surface at [`FLOOR_Z`] over the whole board. The rest stock
/// standing inside the island is the ONLY material above it, which is exactly
/// the shape the model-surface probe cannot see.
fn flat_mesh() -> rs_cam_core::mesh::TriangleMesh {
    common::meshes::height_field_grid(0.0, 1.0, 25, 0.0, 1.0, 25, |_, _| FLOOR_Z)
}

/// The generator shape `dressup::apply_entry` acts on: a rapid to safe Z, a
/// vertical plunge onto the finished surface, then the run being entered.
fn plunge_then_cut() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(ENTRY_X, ENTRY_Y, SAFE_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(
        P3::new(ENTRY_X, ENTRY_Y, FLOOR_Z),
        PLUNGE_RATE,
        MoveIntent::EntryPlunge,
    );
    let mut x = ENTRY_X + 0.25;
    while x <= 14.0 {
        tp.feed_to_with_intent(
            P3::new(x, ENTRY_Y, FLOOR_Z),
            CUT_FEED,
            MoveIntent::FinishingCut,
        );
        x += 0.25;
    }
    tp
}

/// Drive the entry dressup with or without a rest stock on the probe.
fn dressed_entry(stock: &TriDexelStock, carry_rest_stock: bool) -> Toolpath {
    let mesh = flat_mesh();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = tapered_ball();
    let cfg = DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ramp_angle: 3.0,
        arc_fitting: false,
        segment_merge: false,
        link_moves: false,
        lead_in_out: false,
        optimize_rapid_order: false,
        feed_optimization: false,
        ..DressupConfig::default()
    };
    let probe = EntrySurfaceProbe {
        mesh: &mesh,
        index: &index,
        cutter: &cutter,
        stock_to_leave: 0.0,
        off_mesh: OffMeshEntry::PlungeFallback,
        rest_stock: carry_rest_stock.then_some(stock),
    };
    apply_dressups(
        AnnotatedToolpath::new(plunge_then_cut()),
        &cfg,
        CUT_FEED,
        // WP22: no operation in scope, so the plunge cap does not apply.
        None,
        cutter.diameter(),
        SAFE_Z,
        STOCK_TOP_Z,
        None,
        None,
        Some(&cutter),
        Some(probe),
        OperationType::Scallop.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::empty(),
    )
    .toolpath
}

#[test]
fn e_the_dressup_ramp_is_bite_budgeted_over_rest() {
    let stock = rest_stock(true);
    let cutter = tapered_ball();
    let contact = tip_contact_radius(&cutter);
    let tp = dressed_entry(&stock, true);

    assert!(
        count_intent(&tp, MoveIntent::EntryRamp) > 0,
        "the entry dressup emitted no ramp: {:?}",
        tp.moves
    );
    let budget = entry_bite_budget_mm(contact);
    let step = worst_entry_step_mm(&tp, &stock, contact);
    assert!(
        step <= 2.0 * budget + TOL_MM,
        "the dressup ramp took {step:.3} mm off one column, past the \
         {:.3} mm bound",
        2.0 * budget
    );
}

#[test]
fn f_without_the_rest_stock_the_dressup_ramp_takes_the_lot() {
    // RED ARM, kept green by asserting the DEFECT. `emit_ramp`'s legs are
    // clipped to the MODEL surface (G-RAMPTERRAIN), which on a rest-driven pass
    // sits BELOW the material, so nothing lifts them and the closing leg
    // returns to the start column at full depth.
    let stock = rest_stock(true);
    let cutter = tapered_ball();
    let contact = tip_contact_radius(&cutter);
    let tp = dressed_entry(&stock, false);

    let step = worst_entry_step_mm(&tp, &stock, contact);
    assert!(
        step >= REST_DEPTH_MM - TOL_MM,
        "pre-fix reproduction lost: the dressup ramp took only {step:.3} mm \
         off one column, of the {REST_DEPTH_MM:.3} mm standing there"
    );
}

// ── The abstention fall-through (G-ISOCLIPRAMPFALL) ─────────────────────

/// Where the rest stock starts standing, measured along the cut direction.
///
/// Far enough past the entry column that the ladder planner's own window —
/// [`rs_cam_core::pencil::entry_ramp_window_mm`], 1.18 mm on this tip, plus
/// the conservative disc — reads clear ground and abstains. Close enough that
/// the legacy legs walk right over it.
const STEP_X: f64 = 11.5;

/// A model wide enough to hold the legacy legs. They run
/// `ENTRY_CLEARANCE / tan(angle)` mm, which is 38 mm at the shipped 3 degrees,
/// so the 24 mm board of [`flat_mesh`] would lose surface contact and take the
/// clip's own plunge fallback instead of the legs this arm is about.
fn wide_flat_mesh() -> rs_cam_core::mesh::TriangleMesh {
    common::meshes::height_field_grid(0.0, 2.0, 21, 0.0, 2.0, 21, |_, _| FLOOR_Z)
}

/// Ground cut to [`FLOOR_Z`] around the entry column, with the island still
/// standing from [`STEP_X`] on.
fn rest_stock_step() -> TriDexelStock {
    let mut stock =
        TriDexelStock::from_stock(0.0, 0.0, 24.0, 24.0, STOCK_BOTTOM_Z, STOCK_TOP_Z, CELL_MM);
    let (rows, cols) = (stock.z_grid.rows, stock.z_grid.cols);
    let (cs, ou, ov) = (
        stock.z_grid.cell_size,
        stock.z_grid.origin_u,
        stock.z_grid.origin_v,
    );
    for row in 0..rows {
        let y = ov + row as f64 * cs;
        for col in 0..cols {
            let x = ou + col as f64 * cs;
            let inside =
                (STEP_X..=ISLAND_MAX).contains(&x) && (ISLAND_MIN..=ISLAND_MAX).contains(&y);
            let top = if inside { STOCK_TOP_Z } else { FLOOR_Z };
            stock.clear_above_at(row, col, top as f32);
        }
    }
    stock
}

/// [`dressed_entry`] over [`wide_flat_mesh`], so the legacy legs stay on the
/// model instead of taking the clip's plunge fallback.
fn dressed_entry_wide(stock: &TriDexelStock) -> Toolpath {
    let mesh = wide_flat_mesh();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = tapered_ball();
    let cfg = DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ramp_angle: 3.0,
        arc_fitting: false,
        segment_merge: false,
        link_moves: false,
        lead_in_out: false,
        optimize_rapid_order: false,
        feed_optimization: false,
        ..DressupConfig::default()
    };
    let probe = EntrySurfaceProbe {
        mesh: &mesh,
        index: &index,
        cutter: &cutter,
        stock_to_leave: 0.0,
        off_mesh: OffMeshEntry::PlungeFallback,
        rest_stock: Some(stock),
    };
    apply_dressups(
        AnnotatedToolpath::new(plunge_then_cut()),
        &cfg,
        CUT_FEED,
        // WP22: no operation in scope, so the plunge cap does not apply.
        None,
        cutter.diameter(),
        SAFE_Z,
        STOCK_TOP_Z,
        None,
        None,
        Some(&cutter),
        Some(probe),
        OperationType::Scallop.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::empty(),
    )
    .toolpath
}

#[test]
fn g_an_abstaining_planner_plunges_instead_of_walking_the_legacy_legs() {
    // The wanaka200 residual, at fixture scale. The ladder planner reads its
    // own 1.18 mm window, finds nothing over it that the budget does not
    // already cover, and abstains. `emit_ramp` used to fall through to the
    // legacy two-leg zigzag, which is clipped to the MODEL floor only — and
    // on a rest-driven pass that floor lies BELOW the material. Measured
    // pre-fix here at 1.309 mm off one column, against a 1.000 mm bound; on the
    // board at 0.2 mm it was 1.394 mm.
    let stock = rest_stock_step();
    let cutter = tapered_ball();
    let contact = tip_contact_radius(&cutter);

    // Preconditions, so the arm cannot pass on a fixture that says nothing.
    let over_window = stock
        .max_conservative_top_z_in_disc(ENTRY_X, ENTRY_Y, contact)
        .unwrap_or(FLOOR_Z);
    assert!(
        over_window <= FLOOR_Z + TOL_MM,
        "the fixture stands {over_window:.3} mm over the entry column, so the \
         planner would ramp rather than abstain"
    );
    let along_the_leg = stock
        .max_conservative_top_z_in_disc(14.0, ENTRY_Y, contact)
        .unwrap_or(FLOOR_Z);
    assert!(
        along_the_leg >= STOCK_TOP_Z - TOL_MM,
        "the fixture leaves only {along_the_leg:.3} mm standing where the \
         legacy legs walk, so there is nothing for them to gouge"
    );

    let tp = dressed_entry_wide(&stock);
    let entries =
        count_intent(&tp, MoveIntent::EntryRamp) + count_intent(&tp, MoveIntent::EntryPlunge);
    assert!(
        entries > 0,
        "the entry dressup emitted no entry move at all: {:?}",
        tp.moves
    );

    let budget = entry_bite_budget_mm(contact);
    let bite = worst_entry_chord_bite_mm(&tp, &stock, contact);
    assert!(
        bite <= 2.0 * budget + TOL_MM,
        "the abstaining entry took {bite:.3} mm off one column, past the \
         {:.3} mm bound",
        2.0 * budget
    );
}
