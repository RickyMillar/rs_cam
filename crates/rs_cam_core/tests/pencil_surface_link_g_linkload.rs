//! G-LINKLOAD sentry — a pencil LINK must not cut its way across.
//!
//! ## The defect
//!
//! `pencil::emit_paths_with_entry_stock` joins two consecutive valley runs
//! whose XY gap is inside `hookup_distance` with `surface_link::
//! build_surface_link`, which drop-cutters the connecting segment onto the
//! MESH. That is the whole truth only when the mesh IS the material. On a
//! `FromRemainingStock` pencil it is not: material stands wherever the
//! upstream op could not reach, and a link that rides the mesh through it is a
//! CUTTING feed at the tool's full width, tagged `MoveIntent::Linking` so
//! every "is this cutting?" surface reads it as travel.
//!
//! The operator found it on wanaka200 watching the part: *"the pencil is still
//! cutting through some of the mountains in what looks like travel moves"* —
//! an R0.5 tip dragged through up to ~3 mm of standing material mid-link.
//! This is the transit half of the same hole G-ENTRYLOAD closed on entries
//! (`tests/pencil_entry_ramp_g_entryload.rs`), and it shares that fix's bite
//! budget: one number for how much a pencil manoeuvre outside its body cut may
//! remove.
//!
//! ## What is asserted here
//!
//! Everything is measured on **emitted motion**, not on the plan. The
//! instrument is the same 1-D frontier G-ENTRYLOAD uses — the input stock's
//! ceiling per XY, lowered by each fed move that reaches it — so "bite" is
//! exactly what a dexel column under the tip would lose. The ceiling side is
//! recomputed here from the stock the generator was handed, through the same
//! public `max_conservative_top_z_in_disc` query the fix reads, so the gate
//! restates the contract rather than echoing an internal.
//!
//! ## Why it cannot pass vacuously
//!
//! One fixture, one public entry point, three stock arms that differ in
//! NOTHING else:
//!
//! * `None` — no stock reading at all. Must be the legacy emission, and GATE 1
//!   catches it cutting.
//! * cleared — a stock whose material all sits below the mesh. Nothing stands
//!   above the link, so the lift must not engage and the emission must be
//!   move-for-move identical to the `None` arm (GATE 3).
//! * fresh — uncut stock standing over the whole valley, the
//!   `FromRemainingStock` worst case in miniature. GATE 2 requires every
//!   linking sample to clear it.
//!
//! A stub that always lifted would fail GATE 3; one that never lifted would
//! fail GATE 2; one that lifted but dropped the link's job would fail GATE 4's
//! coverage half.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::collections::{HashMap, HashSet};

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::pencil::{
    PencilDetector, PencilParams, entry_bite_budget_mm, pencil_toolpath_structured_annotated,
};
use rs_cam_core::surface_link::build_surface_link;
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::toolpath::{MoveIntent, MoveType, PLUNGE_CLEARANCE_MM, Toolpath};

mod common;

use common::meshes::GroovedBlock;
use common::tools::ball_cutter;

// ── Fixture ─────────────────────────────────────────────────────────────

/// Ø1 ball: `cusp_radius_mm() == 0.5`, the same tip scale as the wanaka
/// pencil's R0.5 tapered ball, so the budget this fixture is graded against is
/// the budget the reported case would get. Shared verbatim with the
/// G-ENTRYLOAD fixture.
const TOOL_DIAMETER_MM: f64 = 1.0;

/// Stock top. The block is uncut in the `fresh` arm, so the conservative
/// ceiling under the tip is this everywhere.
const STOCK_TOP_Z: f64 = 0.0;

/// Where the `cleared` arm's material top is put: below the groove floor
/// (−1.5) and above the stock's own floor (−6.0), so no column stands
/// anywhere near the link's path and the lift has nothing to clear.
const CLEARED_TOP_Z: f64 = -3.0;

/// Safe Z for the arms that expect links to survive.
const SAFE_Z: f64 = 5.0;

/// Safe Z for GATE 5. The lifted transit sits at `0.0 + PLUNGE_CLEARANCE_MM`
/// = 2.0 (uncut stock top, plus the clearance every ceiling in this repo
/// carries), so a safe Z at 1.9 puts the clearance AT OR ABOVE the retract
/// height — the point past which a fed link buys nothing the rapid does not
/// already pay for.
const REFUSAL_SAFE_Z: f64 = 1.9;

/// A 40 x 24 mm block, top at z = 0, with one straight trapezoidal groove
/// along Y: rim half-width 3 mm, 45° walls, floor 1.5 mm down. Identical to
/// the G-ENTRYLOAD fixture, and chosen here for the property that fixture does
/// not use: the groove is a TRAPEZOID, so it has **two** concave creases — the
/// floor/wall corners at x = ±1.5 — and therefore two chains with a junction
/// between them. That junction is the thing under test.
///
/// **Closed form.** A ball of radius `r` traced along a floor/wall crease is
/// tangent to the wall, so its tip rests at `-D + r(1/cos W - 1)` =
/// `-1.5 + 0.5(√2 - 1) = -1.293 mm`. The link between the two creases crosses
/// the flat floor, where the same ball rests at the floor itself, `-1.5 mm`.
/// Against an uncut stock top of 0.0 that is **1.5 mm of standing material the
/// legacy link feeds straight through** — 6x the 0.25 mm budget this tip
/// earns.
fn valley_mesh() -> TriangleMesh {
    GroovedBlock::new(3.0, 45.0, 1.5)
        .dense_half_width(5.0)
        .build()
}

/// Uncut stock, top at z = 0 — the `FromRemainingStock` worst case.
fn fresh_stock() -> TriDexelStock {
    TriDexelStock::from_stock(-20.0, -12.0, 20.0, 12.0, -6.0, STOCK_TOP_Z, 0.25)
}

/// The same stock with every column cut down to [`CLEARED_TOP_Z`]. The
/// control: a stock reading is present, but nothing stands above the link, so
/// a correct fix must leave the emission exactly as it found it.
fn cleared_stock() -> TriDexelStock {
    let mut stock = fresh_stock();
    let rows = stock.z_grid.rows;
    let cols = stock.z_grid.cols;
    for row in 0..rows {
        for col in 0..cols {
            stock.clear_above_at(row, col, CLEARED_TOP_Z as f32);
        }
    }
    stock
}

/// `hookup_distance: 30.0` is deliberately generous: it makes the junction
/// between the two creases a link candidate whichever way the nearest-neighbour
/// orderer chooses to run them (adjacent ends, 3 mm apart, or opposite ends,
/// ~24 mm apart), so the fixture does not quietly stop exercising the defect if
/// that ordering ever changes. GATE 1 is the tripwire if it does.
fn params(safe_z: f64) -> PencilParams {
    PencilParams {
        detector: PencilDetector::Dihedral,
        // Keep the trace on the crease rather than walking it out onto the
        // flat, exactly as the G-ENTRYLOAD fixture does.
        bisector_strength: 0.0,
        min_valley_depth: 0.05,
        min_cut_length: 2.0,
        num_offset_passes: 0,
        sampling: 0.5,
        stock_to_leave: 0.0,
        feed_rate: 1500.0,
        plunge_rate: 150.0,
        safe_z,
        hookup_distance: 30.0,
        reference_tool_diameter: 12.0,
        ..PencilParams::default()
    }
}

/// Generate the pencil pass. `stock` is the ONLY difference between the arms.
fn generate_at(stock: Option<&TriDexelStock>, safe_z: f64) -> Toolpath {
    let mesh = valley_mesh();
    let index = SpatialIndex::build(&mesh, 4.0);
    let cutter = ball_cutter(TOOL_DIAMETER_MM);
    let p = params(safe_z);
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

fn generate(stock: Option<&TriDexelStock>) -> Toolpath {
    generate_at(stock, SAFE_Z)
}

/// The tip radius the fix reads its ceiling over — `corner_radius_mm()` is 0
/// for a ball, so `tip_contact_radius` falls through to the cusp sphere.
fn tip_radius() -> f64 {
    ball_cutter(TOOL_DIAMETER_MM).cusp_radius_mm()
}

fn budget_mm() -> f64 {
    entry_bite_budget_mm(tip_radius())
}

/// A FED move tagged as a link. The retract branch emits a `Rapid` with the
/// same intent, which is travel by construction and not what this file is
/// about.
fn is_link_feed(m: &rs_cam_core::toolpath::Move) -> bool {
    m.move_type != MoveType::Rapid && m.intent == MoveIntent::Linking
}

// ── Instrument ──────────────────────────────────────────────────────────

/// XY key at 1e-4 mm.
fn key(p: P3) -> (i64, i64) {
    (
        (p.x * 10_000.0).round() as i64,
        (p.y * 10_000.0).round() as i64,
    )
}

/// The worst single bite taken by a LINK move.
///
/// A 1-D dexel: each XY starts at `top_z` and is lowered by every fed move
/// that lands on it, in emission order. Rapids are skipped — a rapid that
/// removed material would be a rapid collision, a different finding with its
/// own channel. The tool's radius is ignored, which can only ever make a
/// reading LARGER (a neighbour's clearing is not credited), so the bound
/// asserted below is conservative.
fn worst_link_bite(tp: &Toolpath, top_z: f64) -> f64 {
    let mut frontier: HashMap<(i64, i64), f64> = HashMap::new();
    let mut worst: f64 = 0.0;
    for m in &tp.moves {
        if m.move_type == MoveType::Rapid {
            continue;
        }
        let top = frontier.entry(key(m.target)).or_insert(top_z);
        let bite = (*top - m.target.z).max(0.0);
        *top = top.min(m.target.z);
        if is_link_feed(m) {
            worst = worst.max(bite);
        }
    }
    worst
}

/// Every CUT position, as comparable keys with Z.
///
/// Link moves are excluded on purpose. The whole point of the fix is that a
/// link stops riding the surface, so its samples MOVE — counting them as
/// coverage would make the coverage gate assert the defect. What must not move
/// is the material the pass is there to remove: body cuts and entries.
fn cut_points(tp: &Toolpath) -> Vec<(i64, i64, i64)> {
    tp.moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid && m.intent != MoveIntent::Linking)
        .map(|m| {
            let (kx, ky) = key(m.target);
            (kx, ky, (m.target.z * 10_000.0).round() as i64)
        })
        .collect()
}

/// Move-for-move comparison key: target, kind and intent. Used to assert two
/// arms emitted the SAME toolpath, not merely similar ones.
fn move_shape(tp: &Toolpath) -> Vec<(i64, i64, i64, MoveType, MoveIntent)> {
    tp.moves
        .iter()
        .map(|m| {
            let (kx, ky) = key(m.target);
            (
                kx,
                ky,
                (m.target.z * 10_000.0).round() as i64,
                m.move_type,
                m.intent,
            )
        })
        .collect()
}

/// Maximal runs of consecutive link feeds, as `(previous move's target, the
/// run's own targets)`. The previous move is where the tool was standing when
/// the link began — i.e. the `from` the generator handed
/// [`build_surface_link`].
fn link_runs(tp: &Toolpath) -> Vec<(P3, Vec<P3>)> {
    let mut runs = Vec::new();
    let mut i = 0usize;
    while i < tp.moves.len() {
        if !is_link_feed(&tp.moves[i]) {
            i += 1;
            continue;
        }
        let start = i;
        let mut pts = Vec::new();
        while i < tp.moves.len() && is_link_feed(&tp.moves[i]) {
            pts.push(tp.moves[i].target);
            i += 1;
        }
        // A link is always preceded by the cut it left, never by nothing:
        // the first run of the pass is entered by rapid + plunge.
        assert!(start > 0, "a link run started at move 0");
        runs.push((tp.moves[start - 1].target, pts));
    }
    runs
}

/// The ceiling contract, recomputed from the stock the generator was handed.
fn material_top(stock: &TriDexelStock, x: f64, y: f64) -> f64 {
    stock
        .max_conservative_top_z_in_disc(x, y, tip_radius())
        .unwrap_or(stock.stock_bbox.max.z)
}

// ── Gates ───────────────────────────────────────────────────────────────

/// GATE 1 (red-first, in-file) — the legacy link really does cut.
///
/// The control that keeps every other gate honest: if the fixture ever stops
/// producing an over-budget legacy link, the rest of this file is proving
/// nothing and this gate says so first.
#[test]
fn g_linkload_the_legacy_link_feeds_through_standing_material() {
    let tp = generate(None);
    let links = tp.moves.iter().filter(|m| is_link_feed(m)).count();
    assert!(
        links > 0,
        "fixture emitted no surface link at all — the two floor/wall creases \
         are no longer being joined, so nothing here measures a link"
    );

    let worst = worst_link_bite(&tp, STOCK_TOP_Z);
    let budget = budget_mm();
    println!(
        "G-LINKLOAD legacy arm: {links} link feeds, worst link bite {worst:.3} mm \
         against a {budget:.3} mm budget ({:.1}x) — closed form says the floor \
         crossing stands 1.500 mm proud",
        worst / budget
    );
    assert!(
        worst > budget * 2.0,
        "the fixture must exercise the defect: the legacy link bit {worst:.3} mm, \
         budget {budget:.3} mm"
    );
}

/// GATE 2 — every link sample of the stock-aware arm clears the standing
/// material under it, by the clearance the ceiling contract names.
///
/// Recomputed from the input stock through the public
/// `max_conservative_top_z_in_disc` — the same query the fix reads, asked
/// again here rather than echoed, so the gate would survive the fix being
/// rewritten and would fail if the disc radius or the clearance changed
/// silently.
#[test]
fn g_linkload_a_stock_aware_link_clears_the_material_under_it() {
    let stock = fresh_stock();
    let tp = generate(Some(&stock));

    let link_moves: Vec<_> = tp.moves.iter().filter(|m| is_link_feed(m)).collect();
    assert!(
        !link_moves.is_empty(),
        "the stock-aware arm emitted no link at all — it must LIFT the link, \
         not delete it"
    );

    let mut worst_shortfall = f64::NEG_INFINITY;
    for m in &link_moves {
        let need = material_top(&stock, m.target.x, m.target.y) + PLUNGE_CLEARANCE_MM;
        worst_shortfall = worst_shortfall.max(need - m.target.z);
        assert!(
            m.target.z >= need - 1e-6,
            "a link feed at ({:.3}, {:.3}) sits at z={:.3}, under the {need:.3} mm \
             clearance the material below it requires",
            m.target.x,
            m.target.y,
            m.target.z
        );
    }
    println!(
        "G-LINKLOAD lifted arm: {} link feeds, worst clearance shortfall \
         {worst_shortfall:.6} mm (<= 0 is clear)",
        link_moves.len()
    );

    // ...and the lift really did something: against an uncut stock the legacy
    // link rode the floor at -1.5, so a link feed at or below the stock top
    // would mean nothing moved.
    let worst = worst_link_bite(&tp, STOCK_TOP_Z);
    assert!(
        worst <= 1e-9,
        "a link feed still removed {worst:.3} mm of standing material"
    );
}

/// GATE 3 — no standing material, no lift.
///
/// Two halves, and both matter. The `None` arm must reproduce the legacy
/// emission exactly: every link run must equal what `build_surface_link`
/// returns for its own endpoints, reconstructed here independently. And a
/// stock whose material lies entirely below the mesh must produce the SAME
/// toolpath as no stock at all — otherwise the lift is firing on the presence
/// of a stock reading rather than on the presence of material, and every
/// pencil junction in the repo just bought a 2 mm hop for nothing.
#[test]
fn g_linkload_without_material_above_it_the_link_is_unchanged() {
    let mesh = valley_mesh();
    let index = SpatialIndex::build(&mesh, 4.0);
    let cutter = ball_cutter(TOOL_DIAMETER_MM);
    let p = params(SAFE_Z);

    let legacy = generate(None);
    let runs = link_runs(&legacy);
    assert!(!runs.is_empty(), "legacy arm emitted no link run");

    for (from, pts) in &runs {
        // The last sample of a stay-down link lands ON the next run's first
        // point; everything before it is `build_surface_link`'s interior.
        let to = *pts.last().unwrap();
        let expected = build_surface_link(
            *from,
            to,
            &mesh,
            &index,
            &cutter,
            p.stock_to_leave,
            p.sampling,
        )
        .expect("the emitted link proves this segment is gouge-safe");
        assert_eq!(
            pts.len(),
            expected.len() + 1,
            "legacy link run has {} samples, `build_surface_link` says {} + the \
             arrival",
            pts.len(),
            expected.len()
        );
        for (got, want) in pts.iter().zip(expected.iter()) {
            assert!(
                (got.x - want.x).abs() < 1e-12
                    && (got.y - want.y).abs() < 1e-12
                    && (got.z - want.z).abs() < 1e-12,
                "legacy link sample moved: got {got:?}, want {want:?}"
            );
        }
    }
    println!(
        "G-LINKLOAD none-stock arm: {} link run(s), every sample identical to \
         `build_surface_link`",
        runs.len()
    );

    let cleared = cleared_stock();
    assert_eq!(
        move_shape(&generate(Some(&cleared))),
        move_shape(&legacy),
        "a stock with nothing standing above the mesh changed the emission — \
         the lift must be gated on material, not on having a stock to read"
    );
}

/// GATE 4 — coverage is sacred.
///
/// Lifting a link converts "shave the ridge en route" into "clear air hop",
/// and the cusps on that ridge belong to the finish op, not to the pencil's
/// transit. What the pencil is actually there to cut must not move: every cut
/// position the no-material control makes is still cut, at the same Z, by the
/// stock-aware arm.
#[test]
fn g_linkload_the_lift_does_not_cost_any_coverage() {
    let cleared = cleared_stock();
    let control = cut_points(&generate(Some(&cleared)));
    let fresh = fresh_stock();
    let lifted: HashSet<_> = cut_points(&generate(Some(&fresh))).into_iter().collect();

    assert!(!control.is_empty(), "control arm cut nothing");
    let missing: Vec<_> = control.iter().filter(|p| !lifted.contains(p)).collect();
    println!(
        "G-LINKLOAD coverage: {} control cut points, {} still cut, {} missing",
        control.len(),
        control.len() - missing.len(),
        missing.len()
    );
    assert!(
        missing.is_empty(),
        "the stock-aware arm dropped {} cut point(s) the control made, e.g. {:?}",
        missing.len(),
        missing.first()
    );
}

/// GATE 5 — a lift that reaches retract height falls back to the retract.
///
/// The lifted transit here sits at `stock top + PLUNGE_CLEARANCE_MM` = 2.0.
/// At [`SAFE_Z`] that is well under the retract plane and the link is kept; at
/// [`REFUSAL_SAFE_Z`] it is not, and a fed link at retract height is strictly
/// worse than the rapid it would replace — so the retract must come back,
/// entry manoeuvre and all. Asserted on move shape, not on a report field.
#[test]
fn g_linkload_a_lift_that_reaches_safe_z_falls_back_to_a_retract() {
    let stock = fresh_stock();

    let retracts = |tp: &Toolpath| -> usize {
        tp.moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid && m.intent == MoveIntent::Retract)
            .count()
    };

    let kept = generate_at(Some(&stock), SAFE_Z);
    let kept_links = kept.moves.iter().filter(|m| is_link_feed(m)).count();
    assert!(
        kept_links > 0,
        "control: with room to clear, the lifted link must survive"
    );

    let refused = generate_at(Some(&stock), REFUSAL_SAFE_Z);
    let refused_links = refused.moves.iter().filter(|m| is_link_feed(m)).count();
    println!(
        "G-LINKLOAD refusal: safe_z {SAFE_Z} keeps {kept_links} link feeds and \
         {} retracts; safe_z {REFUSAL_SAFE_Z} keeps {refused_links} and {} retracts",
        retracts(&kept),
        retracts(&refused)
    );
    assert_eq!(
        refused_links, 0,
        "the clearance reaches safe_z, so there is nothing left for a fed link \
         to save — the retract must be kept"
    );
    // Every pass closes with one retract, so the count alone proves nothing:
    // what proves the fallback is that refusing the links ADDED retracts.
    assert!(
        retracts(&refused) > retracts(&kept),
        "the link was refused but no retract took its place: {} retracts either \
         way",
        retracts(&kept)
    );
}
