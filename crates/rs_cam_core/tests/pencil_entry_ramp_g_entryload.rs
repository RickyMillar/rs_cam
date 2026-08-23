//! G-ENTRYLOAD sentry — a pencil ENTRY must not carve its way in.
//!
//! ## The defect
//!
//! `pencil::emit_paths` entered every run it could not surface-link to the
//! same way: rapid to the run's first point at `safe_z`, then ONE straight
//! fed descent to that point's finished Z. Nothing in that descent knows how
//! much material stands over the crease, so on a `FromRemainingStock` pencil
//! it is a vertical carve through whatever the upstream op left behind. On
//! wanaka200 (240 x 250 x 25 white oak, R0.5 tapered ball, op 9) it measured
//! **3.72 mm removed in one bite at (44.5, 92.1, 1.64)** — 18x the pass's own
//! 0.20 mm median — and it cost **2,412 s of the op's 3,332 s**, because the
//! descent runs at the 150 mm/min tapered-ball flute-tip plunge cap.
//!
//! The operator found it on the part ("ramp entries are cutting through the
//! stock, surprised this isn't flagged"). No surface had said a word, because
//! a pure-vertical descent samples as `CutKinematics::Plunge` and its removal
//! is written to `plunge_descent_mm`, which the crosses-standing rule does
//! not read and the load gates never see (they filter entry spans out by
//! design). That half of the hole is closed by
//! `sim_triage::entry_load_observation` and its own sentries; this file is
//! about the motion.
//!
//! ## What is asserted here
//!
//! Everything is measured on **emitted motion**, not on the plan — the
//! narration's Z ladder is nominal, and the lesson from the last retraction
//! is that "did it cut too deep?" is answered by the moves. The instrument is
//! a 1-D frontier: the input stock's ceiling per path point, lowered by each
//! fed move that reaches it, so "bite" here is exactly what a dexel column
//! under the tip would lose.
//!
//! ## Why it cannot pass vacuously
//!
//! The same fixture is generated twice through the same public entry point,
//! differing ONLY in whether the input stock is handed over
//! (`initial_stock`). The legacy arm must be caught biting more than the
//! budget (red-first, in-file) or the fixture is not exercising the defect;
//! the stock-aware arm must stay inside it. A stub that always ramped, or
//! never did, fails one of the two.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::collections::HashMap;

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::pencil::{
    PencilDetector, PencilParams, entry_bite_budget_mm, pencil_toolpath_structured_annotated,
};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

mod common;

use common::meshes::GroovedBlock;
use common::tools::ball_cutter;

// ── Fixture ─────────────────────────────────────────────────────────────

/// Ø1 ball: `cusp_radius_mm() == 0.5`, the same tip scale as the wanaka
/// pencil's R0.5 tapered ball, so the budget this fixture is graded against
/// is the budget the reported case would get. A ball rather than the taper
/// because `cusp_radius_mm() == envelope_radius_mm()` here, which keeps the
/// closed-form resting depth in the fixture doc arithmetic anyone can check.
const TOOL_DIAMETER_MM: f64 = 1.0;

/// Stock top. The block is uncut, so the conservative ceiling under the tip
/// is this everywhere — i.e. the whole valley is standing material, which is
/// the `FromRemainingStock` worst case in miniature.
const STOCK_TOP_Z: f64 = 0.0;

/// A 40 x 24 mm block, top at z = 0, with one straight trapezoidal groove
/// along Y: rim half-width 3 mm, 45° walls, floor 1.5 mm down. Same generator
/// `pencil_tip_float_channel_d1` proves the DIHEDRAL arm drives a real
/// centreline down, at a wall angle that lets the tool actually descend.
///
/// **Closed form.** A ball of radius `r` traced along the floor/wall crease
/// is tangent to the wall, so its centre sits `r / cos(W)` above the corner
/// and its tip rests at `-D + r(1/cos W - 1)`. Here that is
/// `-1.5 + 0.5(√2 - 1) = -1.293 mm`, i.e. **1.29 mm of standing stock over
/// the entry point** — 5x the 0.25 mm per-lap budget this tip earns, and well
/// clear of the -1.5 mm floor.
///
/// (The same formula is why an 80° groove would NOT do: `1/cos 80° = 5.76`
/// wedges the ball 0.12 mm below the top, inside the budget, and there would
/// be no defect left to measure.)
fn valley_mesh() -> TriangleMesh {
    GroovedBlock::new(3.0, 45.0, 1.5)
        .dense_half_width(5.0)
        .build()
}

fn fresh_stock() -> TriDexelStock {
    TriDexelStock::from_stock(-20.0, -12.0, 20.0, 12.0, -6.0, STOCK_TOP_Z, 0.25)
}

/// `hookup_distance: 0.0` forces the retract/re-enter branch on every run, so
/// every run in this fixture has a real ENTRY to measure (the same isolation
/// `pencil.rs`'s own split tests use).
fn params() -> PencilParams {
    PencilParams {
        detector: PencilDetector::Dihedral,
        // Keep the trace on the crease rather than walking it out onto the
        // flat, exactly as `pencil_tip_float_channel_d1` does.
        bisector_strength: 0.0,
        min_valley_depth: 0.05,
        min_cut_length: 2.0,
        num_offset_passes: 0,
        sampling: 0.5,
        feed_rate: 1500.0,
        plunge_rate: 150.0,
        safe_z: 5.0,
        hookup_distance: 0.0,
        reference_tool_diameter: 12.0,
        ..PencilParams::default()
    }
}

/// Generate the pencil pass. `stock` is the ONLY difference between the two
/// arms: `None` is the pre-G-ENTRYLOAD emission, `Some` is the ramped one.
fn generate(stock: Option<&TriDexelStock>) -> Toolpath {
    let mesh = valley_mesh();
    let index = SpatialIndex::build(&mesh, 4.0);
    let cutter = ball_cutter(TOOL_DIAMETER_MM);
    let p = params();
    let mut grid = None;
    let mut regions = None;
    let (tp, _ann) = pencil_toolpath_structured_annotated(
        &mesh,
        &index,
        &cutter,
        &p,
        stock,
        None,
        &mut grid,
        &mut regions,
    );
    tp
}

fn budget_mm() -> f64 {
    entry_bite_budget_mm(ball_cutter(TOOL_DIAMETER_MM).cusp_radius_mm())
}

fn is_entry(intent: MoveIntent) -> bool {
    matches!(
        intent,
        MoveIntent::EntryPlunge | MoveIntent::EntryRamp | MoveIntent::EntryHelix
    )
}

// ── Instrument ──────────────────────────────────────────────────────────

/// XY key at 1e-4 mm — every point compared here comes from the SAME path
/// slice in both arms, so this only has to survive an exact copy.
fn key(p: P3) -> (i64, i64) {
    (
        (p.x * 10_000.0).round() as i64,
        (p.y * 10_000.0).round() as i64,
    )
}

/// The worst single bite taken by an entry-intent move, and by any move.
///
/// A 1-D dexel: each XY starts at the stock ceiling and is lowered by every
/// fed move that lands on it. Rapids are skipped — a rapid that removed
/// material would be a rapid collision, which is a different finding with its
/// own channel. The tool's radius is ignored, which can only ever make a
/// reading LARGER (a neighbour's clearing is not credited), so the bound
/// asserted below is conservative.
fn worst_bites(tp: &Toolpath) -> (f64, f64) {
    let mut frontier: HashMap<(i64, i64), f64> = HashMap::new();
    let mut worst_entry: f64 = 0.0;
    let mut worst_any: f64 = 0.0;
    for m in &tp.moves {
        if m.move_type == MoveType::Rapid {
            continue;
        }
        let top = frontier.entry(key(m.target)).or_insert(STOCK_TOP_Z);
        let bite = (*top - m.target.z).max(0.0);
        *top = top.min(m.target.z);
        worst_any = worst_any.max(bite);
        if is_entry(m.intent) {
            worst_entry = worst_entry.max(bite);
        }
    }
    (worst_entry, worst_any)
}

/// Commanded time (s) of the entry-intent moves, under each move's own feed.
/// A model, not a measurement — there is no accel here — but the same model
/// on both arms, which is what a ratio needs.
fn entry_time_s(tp: &Toolpath) -> f64 {
    let mut total = 0.0;
    let mut prev: Option<P3> = None;
    for m in &tp.moves {
        if let (Some(from), Some(feed)) = (prev, m.move_type.feed_rate())
            && is_entry(m.intent)
            && feed > 0.0
        {
            let d = ((m.target.x - from.x).powi(2)
                + (m.target.y - from.y).powi(2)
                + (m.target.z - from.z).powi(2))
            .sqrt();
            total += d / feed * 60.0;
        }
        prev = Some(m.target);
    }
    total
}

/// Every fed move endpoint, as comparable keys with Z.
fn cut_points(tp: &Toolpath) -> Vec<(i64, i64, i64)> {
    tp.moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid)
        .map(|m| {
            let (kx, ky) = key(m.target);
            (kx, ky, (m.target.z * 10_000.0).round() as i64)
        })
        .collect()
}

// ── Gates ───────────────────────────────────────────────────────────────

/// GATE 1 (red-first, in-file) — the legacy entry really does carve.
///
/// This is the control that keeps GATE 2 honest: if the fixture ever stops
/// producing an over-budget legacy entry, GATE 2 is proving nothing and this
/// gate says so first.
#[test]
fn g_entryload_the_legacy_entry_carves_through_standing_material() {
    let tp = generate(None);
    assert!(
        tp.moves.iter().any(|m| is_entry(m.intent)),
        "fixture emitted no entries at all"
    );
    let (worst_entry, _) = worst_bites(&tp);
    let budget = budget_mm();
    println!(
        "G-ENTRYLOAD legacy arm: worst entry bite {worst_entry:.3} mm against a \
         {budget:.3} mm budget ({:.1}x) — closed form says 1.293 mm",
        worst_entry / budget
    );
    assert!(
        worst_entry > budget * 2.0,
        "the fixture must exercise the defect: legacy entry bit {worst_entry:.3} mm, \
         budget {budget:.3} mm"
    );
}

/// GATE 2 — the stock-aware entry stays inside its per-lap bite budget.
///
/// `2 x step <= budget` is the design bound (two consecutive zig-zag laps are
/// furthest apart at the turn), so the assertion is the budget itself, not
/// the step.
#[test]
fn g_entryload_a_ramped_entry_stays_inside_its_bite_budget() {
    let stock = fresh_stock();
    let tp = generate(Some(&stock));
    let budget = budget_mm();

    let ramp_moves = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryRamp)
        .count();
    assert!(
        ramp_moves > 0,
        "no entry ramp was emitted — the stock-aware path did not engage"
    );

    let (worst_entry, worst_any) = worst_bites(&tp);
    println!(
        "G-ENTRYLOAD ramped arm: {ramp_moves} ramp moves, worst entry bite \
         {worst_entry:.3} mm, worst bite anywhere {worst_any:.3} mm, budget \
         {budget:.3} mm"
    );
    assert!(
        worst_entry <= budget + 1e-6,
        "an entry-intent move removed {worst_entry:.3} mm, over the {budget:.3} mm \
         budget"
    );
    // ...and the body is untouched by this change: the pass still takes the
    // full remaining depth in its steady cutting, which is what the
    // crosses-standing rule reports and what this fix deliberately does NOT
    // move. If this ever equalled the entry bound, the entry would have been
    // "fixed" by making the whole pass shallow.
    assert!(
        worst_any > budget,
        "the body must still cut at full depth ({worst_any:.3} mm) — otherwise \
         this fixture is no longer measuring an ENTRY"
    );
}

/// GATE 3 — coverage is sacred. Every point the legacy arm cut is still cut,
/// at the same Z, by the ramped arm.
///
/// The campaign that took pencil valley coverage from 0.137 to 0.80 is not
/// being paid back a wedge per entry: the ramp's final lap is flat at the
/// window floor precisely so the window ends up machined to the same Z the
/// body pass would have left.
#[test]
fn g_entryload_the_ramp_does_not_cost_any_coverage() {
    let legacy = cut_points(&generate(None));
    let stock = fresh_stock();
    let ramped: std::collections::HashSet<_> =
        cut_points(&generate(Some(&stock))).into_iter().collect();

    assert!(!legacy.is_empty(), "legacy arm cut nothing");
    let missing: Vec<_> = legacy.iter().filter(|p| !ramped.contains(p)).collect();
    println!(
        "G-ENTRYLOAD coverage: {} legacy cut points, {} still cut, {} missing",
        legacy.len(),
        legacy.len() - missing.len(),
        missing.len()
    );
    assert!(
        missing.is_empty(),
        "the ramped arm dropped {} cut point(s) the legacy arm made, e.g. {:?}",
        missing.len(),
        missing.first()
    );
}

/// GATE 4 — entry time is at worst neutral.
///
/// The ramp trades a 150 mm/min vertical descent through material for
/// lateral travel at cutting feed plus an air-only descent to the stock
/// ceiling. Commanded time only (no accel), same model both sides.
#[test]
fn g_entryload_the_ramp_does_not_cost_entry_time() {
    let legacy = entry_time_s(&generate(None));
    let stock = fresh_stock();
    let ramped = entry_time_s(&generate(Some(&stock)));
    println!(
        "G-ENTRYLOAD entry time (commanded): legacy {legacy:.2} s, ramped \
         {ramped:.2} s ({:.2}x)",
        ramped / legacy
    );
    assert!(legacy > 0.0, "legacy arm spent no time entering");
    assert!(
        ramped <= legacy * 1.5,
        "ramped entry time {ramped:.2} s is more than 1.5x the legacy \
         {legacy:.2} s — the ramp geometry has stopped paying for itself"
    );
}

/// GATE 5 — no stock reading, no ramp. The generator must not invent a
/// ceiling it cannot see: without an input stock the legacy descent is kept,
/// unchanged, which is what makes GATE 1's control a control.
#[test]
fn g_entryload_without_a_stock_reading_the_emission_is_unchanged() {
    let a = cut_points(&generate(None));
    let b = cut_points(&generate(None));
    assert_eq!(a, b, "generation must be deterministic");
    let tp = generate(None);
    assert!(
        !tp.moves.iter().any(|m| m.intent == MoveIntent::EntryRamp),
        "a ramp was emitted with no stock to measure the ceiling against"
    );
}
