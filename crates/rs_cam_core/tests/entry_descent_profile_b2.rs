//! B2 — the profile-aware entry-descent target.
//!
//! ## The defect
//!
//! [`rs_cam_core::dressup::optimize_entry_descents`] splits a long
//! safe-Z-to-cut-depth plunge by rapiding down to just above the input
//! stock's material ceiling first. It read that ceiling with
//! `max_conservative_top_z_in_disc` over a disc of the tool's ENVELOPE
//! radius — the cutter modelled as a flat cylinder. Exact for a flat
//! endmill; wrong for every other profile, because past its tip a real
//! cutter RISES, so material at lateral offset `r` can only strike it if it
//! stands more than `height_at_radius(r)` above the tip.
//!
//! This is the direct sibling of the stay-down link ceiling Track A1 fixed
//! (`tests/profile_link_ceiling.rs`), and the surplus is more expensive here:
//! a link's surplus is flown at rapid rate, an entry descent's is fed at
//! PLUNGE rate. Phase S3 (`planning/rapid_safety_2026-08-28/S3_RESULTS.md`)
//! restored fed plunges through crest material and cost +9.6% on the wanaka
//! cycle estimate; this recovers the part of that which was never in reach of
//! the cutter in the first place.
//!
//! ## The rule, and what changed
//!
//! ```text
//! DECISION (unchanged):  split iff  flat_ceiling + CLEARANCE  is
//!                        <= rapid_z - 0.5  and  > plunge_target_z
//! TARGET   (B2):         max over r in [0, envelope_radius] of
//!                          [ material_top_at(r) - height_at_radius(r) ]
//!                        + CLEARANCE, clamped to the flat answer
//! ```
//!
//! **HEIGHT, not DECISION.** The pass must fire on exactly the entries it
//! always did — only the inserted rapid's Z may move, and only downward.
//!
//! ## What is asserted here
//!
//! * GATE 1 — the SAFETY ANCHOR, on the EMITTED TOOLPATH. For a flat endmill
//!   `height_at_radius(r) == Some(0.0)` throughout the envelope, so every
//!   move — count, type, intent and all three coordinates — must equal the
//!   pre-B2 expectation, rebuilt here from the old primitive. Compared with
//!   `==` on `f64`. Mirrors
//!   `profile_link_ceiling::flat_endmill_profile_ceiling_is_byte_identical`,
//!   one level further out.
//! * GATE 2 — a tapered ball descends LOWER over a ridge it cannot touch,
//!   and by the analytically-predicted amount (the ridge's whole standing
//!   height), while the split count is unmoved.
//! * GATE 3 — it does NOT descend lower where the flank would strike: a
//!   ridge under the tip moves the target by zero ULPs, and a ridge inside
//!   the BALL relaxes by at most the ball's own rise.
//! * GATE 4 — the falsification pairing. The tightened descent is replayed
//!   through the S2 LIVE profile-aware rapid check
//!   ([`RapidClearanceCheck`], riding the simulator's own walk) and must
//!   report ZERO strikes — with a non-vacuity guard that the optimization
//!   actually fired and actually moved the target down, and an awake-detector
//!   guard that the same check on the same stock DOES flag a descent that
//!   really does drive the flank into the crest.
//!
//! ```text
//! cargo test -p rs_cam_core --test entry_descent_profile_b2
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::dressup::optimize_entry_descents;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::stock::collision::RapidClearanceCheck;
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::{FlatEndmill, MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::{MoveIntent, MoveType, PLUNGE_CLEARANCE_MM, Toolpath};

// ── Fixture ─────────────────────────────────────────────────────────────

/// 0.2 mm cells, matching `rapid_live_check_crest_s2.rs` so GATE 4's live
/// check runs at the tolerance that file's header documents (≈ 0.441 mm,
/// 2.207 × cell) — comfortably inside the 2.0 mm plunge clearance.
const CELL_MM: f64 = 0.2;

/// Raw stock top, and therefore the top of every standing ridge below.
const STOCK_TOP_Z: f64 = 2.0;
const STOCK_BOTTOM_Z: f64 = -5.0;

/// What the terrain around a ridge is cut down to.
const FLOOR_Z: f64 = 0.0;

/// A ridge therefore stands this tall above its surroundings.
const RIDGE_HEIGHT_MM: f64 = STOCK_TOP_Z - FLOOR_Z;

/// Half-width of a ridge band.
const RIDGE_HALF_WIDTH_MM: f64 = 0.3;

/// Lateral offset of the OUT-OF-REACH ridge: inside the 3.0 mm envelope the
/// flat disc reads, far outside anything the taper's flank can touch.
const RIDGE_OFFSET_MM: f64 = 2.5;

/// The entry the optimizer is pointed at.
const QX: f64 = 10.0;
const QY: f64 = 10.0;
const SAFE_Z: f64 = 12.0;
const PLUNGE_TARGET_Z: f64 = -3.0;

/// The analytic fresh-stock top the caller supplies as the off-grid fallback.
const FRESH_TOP_Z: f64 = STOCK_TOP_Z;

/// Restated from `dressup.rs`'s private `MIN_SPLIT_MM`. Restating it is the
/// point: GATE 1 rebuilds the pre-B2 DECISION from first principles, so a
/// change to the gate has to be mirrored here deliberately rather than
/// inherited silently.
const MIN_SPLIT_MM: f64 = 0.5;

/// The shipped R1.0 tapered ball from the operator's board: ball Ø2.0,
/// 5.7° half-angle, Ø6 shank. `envelope_radius_mm() == 3.0`. Same tool
/// `profile_link_ceiling.rs` measures A1 with, so the two gates are
/// comparable.
fn tapered_ball() -> TaperedBallEndmill {
    TaperedBallEndmill::new(2.0, 5.7, 6.0, 25.0)
}

/// A Ø6 flat endmill — the SAME 3.0 mm envelope radius as [`tapered_ball`],
/// so GATE 1 and GATE 2 read the same disc and differ only in the profile
/// inside it.
fn flat_endmill() -> FlatEndmill {
    FlatEndmill::new(6.0, 25.0)
}

fn blank_stock() -> TriDexelStock {
    TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, STOCK_BOTTOM_Z, STOCK_TOP_Z, CELL_MM)
}

/// Set every column's top from a closure of `(x, y)`.
fn shaped_stock(top_at: &dyn Fn(f64, f64) -> f64) -> TriDexelStock {
    let mut stock = blank_stock();
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
            stock.clear_above_at(row, col, top_at(x, y) as f32);
        }
    }
    stock
}

/// Terrain cut to [`FLOOR_Z`] everywhere except one ridge band standing at the
/// raw stock top, `offset_mm` to the +X side of the entry point and running
/// the full Y extent — the inter-pass crest shape S1/S2 measured.
fn ridge_stock(offset_mm: f64) -> TriDexelStock {
    shaped_stock(&|x, _y| {
        if (x - (QX + offset_mm)).abs() <= RIDGE_HALF_WIDTH_MM {
            STOCK_TOP_Z
        } else {
            FLOOR_Z
        }
    })
}

/// Irregular terrain with no symmetry, for GATE 1: a stub that special-cased
/// flat ground would pass on a plane and fail here.
fn lumpy_stock() -> TriDexelStock {
    shaped_stock(&|x, y| {
        let h = (x * 0.9).sin() * 1.7 + (y * 1.3).cos() * 1.1 + (x * y * 0.21).sin() * 0.8;
        (h - 1.0).clamp(STOCK_BOTTOM_Z + 0.5, STOCK_TOP_Z)
    })
}

/// Entry points spread across the board, including two whose disc hangs off
/// the grid edge so the fallback arm is exercised too.
fn probe_points() -> Vec<(f64, f64)> {
    let mut pts = Vec::new();
    let mut x = 0.5;
    while x <= 19.5 {
        let mut y = 0.5;
        while y <= 19.5 {
            pts.push((x, y));
            y += 3.1;
        }
        x += 3.7;
    }
    pts.push((QX, QY));
    pts.push((-0.4, 10.13));
    pts.push((10.13, 20.4));
    pts
}

/// The `[Rapid to safe_z] -> [Linear EntryPlunge, same XY, descending]`
/// pattern every generator's shared emitter produces — the only shape
/// `optimize_entry_descents` acts on.
fn entry_toolpath(x: f64, y: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(x, y, SAFE_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(
        P3::new(x, y, PLUNGE_TARGET_Z),
        500.0,
        MoveIntent::EntryPlunge,
    );
    tp
}

/// Run the optimizer on one entry and return `(split_count, toolpath)`.
fn optimize_one(
    stock: &TriDexelStock,
    x: f64,
    y: f64,
    cutter: &dyn MillingCutter,
) -> (usize, Toolpath) {
    let mut tp = entry_toolpath(x, y);
    let splits = optimize_entry_descents(
        &mut tp,
        Some(stock),
        FRESH_TOP_Z,
        cutter.envelope_radius_mm(),
        cutter,
        None,
    );
    (splits, tp)
}

/// The Z of the inserted linking rapid, for a toolpath that was split.
fn inserted_descent_z(tp: &Toolpath) -> f64 {
    assert_eq!(tp.moves.len(), 3, "not a split entry: {:?}", tp.moves);
    assert_eq!(tp.moves[1].move_type, MoveType::Rapid);
    assert_eq!(tp.moves[1].intent, MoveIntent::Linking);
    tp.moves[1].target.z
}

/// The PRE-B2 answer, rebuilt from the primitive B2 replaced: the flat-disc
/// material ceiling plus the plunge clearance. This is both the decision
/// input and, for a flat cutter, the whole expected target.
fn flat_target_z(stock: &TriDexelStock, x: f64, y: f64, radius: f64) -> f64 {
    stock
        .max_conservative_top_z_in_disc(x, y, radius)
        .unwrap_or(FRESH_TOP_Z)
        + PLUNGE_CLEARANCE_MM
}

/// The pre-B2 DECISION, restated. Must not move for any input.
fn pre_b2_fires(target_z: f64) -> bool {
    target_z <= SAFE_Z - MIN_SPLIT_MM && target_z > PLUNGE_TARGET_Z
}

// ── GATE 1: the safety anchor, on the emitted toolpath ──────────────────

/// A FLAT endmill's `height_at_radius` is `Some(0.0)` for every radius inside
/// its envelope, so the profile rule reduces exactly to the flat disc and the
/// emitted motion must not move by a single ULP.
///
/// This asserts the TOOLPATH, not the ceiling: move count, move types,
/// intents, and all three coordinates of every move, against an expectation
/// rebuilt from the old primitive. A stub that always relaxes fails here.
#[test]
fn a_flat_endmill_emits_a_byte_identical_toolpath() {
    let flat = flat_endmill();
    let radius = flat.envelope_radius_mm();
    assert!(
        (radius - 3.0).abs() < 1e-12,
        "fixture drift: the flat control must share the tapered ball's 3.0 mm \
         envelope radius, got {radius}"
    );

    let stocks = [
        ("lumpy", lumpy_stock()),
        ("ridge", ridge_stock(RIDGE_OFFSET_MM)),
    ];
    let mut compared = 0usize;
    let mut split_total = 0usize;
    for (name, stock) in &stocks {
        for (x, y) in probe_points() {
            let expected_z = flat_target_z(stock, x, y, radius);
            let fires = pre_b2_fires(expected_z);
            let (splits, tp) = optimize_one(stock, x, y, &flat);

            assert_eq!(
                splits,
                usize::from(fires),
                "{name} at ({x:.3}, {y:.3}): the split DECISION moved for a \
                 flat endmill. B2 changes the descent HEIGHT only — the pass \
                 must fire on exactly the entries it always did."
            );

            let reference = entry_toolpath(x, y);
            if fires {
                assert_eq!(tp.moves.len(), 3, "{name} at ({x:.3}, {y:.3})");
                assert_moves_equal(&tp.moves[0], &reference.moves[0], name, x, y, 0);
                assert_eq!(tp.moves[1].move_type, MoveType::Rapid);
                assert_eq!(tp.moves[1].intent, MoveIntent::Linking);
                assert_eq!(tp.moves[1].target.x, x);
                assert_eq!(tp.moves[1].target.y, y);
                assert_eq!(
                    tp.moves[1].target.z, expected_z,
                    "{name} at ({x:.3}, {y:.3}): the inserted descent moved for \
                     a FLAT endmill. height_at_radius is 0 everywhere inside a \
                     flat envelope, so there is nothing for the profile to relax."
                );
                assert_moves_equal(&tp.moves[2], &reference.moves[1], name, x, y, 2);
                split_total += 1;
            } else {
                assert_eq!(tp.moves.len(), 2, "{name} at ({x:.3}, {y:.3})");
                assert_moves_equal(&tp.moves[0], &reference.moves[0], name, x, y, 0);
                assert_moves_equal(&tp.moves[1], &reference.moves[1], name, x, y, 1);
            }
            compared += 1;
        }
    }
    assert!(
        compared > 50,
        "control: only {compared} entries compared, the sweep collapsed"
    );
    assert!(
        split_total > 25,
        "control: only {split_total} of {compared} entries actually split — a \
         sweep that never exercises the inserted move proves nothing"
    );
}

fn assert_moves_equal(
    got: &rs_cam_core::toolpath::Move,
    want: &rs_cam_core::toolpath::Move,
    stock: &str,
    x: f64,
    y: f64,
    idx: usize,
) {
    assert_eq!(
        got.move_type, want.move_type,
        "{stock} at ({x:.3}, {y:.3}) move {idx}: type"
    );
    assert_eq!(
        got.intent, want.intent,
        "{stock} at ({x:.3}, {y:.3}) move {idx}: intent"
    );
    assert_eq!(
        (got.target.x, got.target.y, got.target.z),
        (want.target.x, want.target.y, want.target.z),
        "{stock} at ({x:.3}, {y:.3}) move {idx}: position"
    );
}

// ── GATE 2: the taper descends lower over a ridge it cannot touch ───────

/// The defect itself. A ridge 2.5 mm off-axis standing 2.0 mm above the floor
/// cannot reach a cutter that stands ~20 mm above its own tip out there, so
/// it must not hold the descent up.
///
/// Direction AND magnitude: the relaxation must equal the ridge's whole
/// standing height, which is what the analytic profile predicts, not merely
/// be "lower".
#[test]
fn a_taper_descends_lower_over_a_ridge_it_cannot_touch() {
    let tool = tapered_ball();
    let flat = flat_endmill();
    let radius = tool.envelope_radius_mm();
    let stock = ridge_stock(RIDGE_OFFSET_MM);

    // Control: the ridge really is inside the disc the old query reads, and
    // really is out of the cutter's reach.
    assert!(
        RIDGE_OFFSET_MM + RIDGE_HALF_WIDTH_MM <= radius,
        "fixture: the ridge must lie inside the {radius} mm search disc"
    );
    let rise = tool
        .height_at_radius(RIDGE_OFFSET_MM - RIDGE_HALF_WIDTH_MM)
        .expect("the ridge is inside the envelope, so the profile has a height there");
    assert!(
        rise > RIDGE_HEIGHT_MM,
        "fixture: the cutter stands {rise:.3} mm above its tip at the ridge's \
         inner edge, which must exceed the ridge's {RIDGE_HEIGHT_MM:.3} mm of \
         standing material or this gate is not measuring the defect"
    );

    let (flat_splits, flat_tp) = optimize_one(&stock, QX, QY, &flat);
    let (taper_splits, taper_tp) = optimize_one(&stock, QX, QY, &tool);

    assert_eq!(
        (flat_splits, taper_splits),
        (1, 1),
        "both tools must split this entry — B2 is a height change, not a \
         decision change"
    );

    let flat_z = inserted_descent_z(&flat_tp);
    let taper_z = inserted_descent_z(&taper_tp);

    assert_eq!(
        flat_z,
        STOCK_TOP_Z + PLUNGE_CLEARANCE_MM,
        "control: the flat disc must stop at the RIDGE top + clearance — the \
         fixture is not reproducing the over-reach"
    );
    assert!(
        (taper_z - (FLOOR_Z + PLUNGE_CLEARANCE_MM)).abs() < 1e-9,
        "the taper's descent must reach the floor {FLOOR_Z:.3} + \
         {PLUNGE_CLEARANCE_MM:.3} clearance (the only material it can actually \
         reach), got {taper_z:.6}"
    );
    assert!(
        flat_z - taper_z >= RIDGE_HEIGHT_MM - 1e-9,
        "non-vacuity: the descent must drop by the ridge's whole height, \
         {RIDGE_HEIGHT_MM:.3} mm; measured {:.6} mm",
        flat_z - taper_z
    );
    // And it never goes UP, which is the only direction that could be unsafe.
    assert!(
        taper_z <= flat_z,
        "the profile rule may only ever lower the descent: {taper_z:.6} > {flat_z:.6}"
    );
}

// ── GATE 3: no relaxation where the flank would strike ──────────────────

/// Material the tool CAN hit must still hold the descent up. Two arms:
///
/// * a ridge straight under the tip (`r = 0`, where `height_at_radius` is 0
///   for every profile) — the target must be EXACTLY the flat answer;
/// * a ridge inside the ball region — the target may relax, but by at most
///   the ball's own rise, and nowhere near the shank relaxation GATE 2
///   measured. That bound is the statement "the relaxation cannot reach
///   material the tool can hit".
#[test]
fn a_ridge_the_taper_can_strike_still_holds_the_descent_up() {
    let tool = tapered_ball();
    let flat = flat_endmill();
    let ball_radius = tool.ball_diameter / 2.0;

    // Arm A — ridge straight under the tip.
    let under_tip = ridge_stock(0.0);
    let (_, flat_a) = optimize_one(&under_tip, QX, QY, &flat);
    let (_, taper_a) = optimize_one(&under_tip, QX, QY, &tool);
    assert_eq!(
        inserted_descent_z(&taper_a),
        inserted_descent_z(&flat_a),
        "material under the TIP is at r = 0, where every profile has height 0: \
         the descent must not move by a single ULP"
    );

    // Arm B — ridge inside the ball region.
    let inner_offset = ball_radius * 0.5;
    assert!(
        inner_offset + RIDGE_HALF_WIDTH_MM < ball_radius,
        "fixture: the inner ridge must lie wholly inside the ball region"
    );
    let inner = ridge_stock(inner_offset);
    let (splits_b, taper_b) = optimize_one(&inner, QX, QY, &tool);
    let (_, flat_b) = optimize_one(&inner, QX, QY, &flat);
    assert_eq!(splits_b, 1, "the fixture must exercise the optimizer");

    let taper_z = inserted_descent_z(&taper_b);
    let flat_z = inserted_descent_z(&flat_b);
    let ball_rise = tool
        .height_at_radius(ball_radius)
        .expect("the ball edge is inside the envelope");

    assert!(
        taper_z <= flat_z + 1e-12,
        "the profile rule may only ever relax: {taper_z:.6} > {flat_z:.6}"
    );
    assert!(
        flat_z - taper_z <= ball_rise + 1e-9,
        "a ridge the BALL can strike lowered the descent by {:.6} mm, more \
         than the ball's own {ball_rise:.6} mm rise — the relaxation reached \
         material the tool can hit",
        flat_z - taper_z
    );
    assert!(
        flat_z - taper_z < RIDGE_HEIGHT_MM * 0.25,
        "an in-ball ridge lowered the descent by {:.3} mm, comparable to the \
         out-of-reach case — the two regimes must not blur",
        flat_z - taper_z
    );
}

// ── GATE 4: the live-check falsification pairing ────────────────────────

/// Run the metric walk — the production seam — with the S2 live check
/// attached, and return the move indices it flagged.
fn live_hits(stock: &mut TriDexelStock, tp: &Toolpath, cutter: &dyn MillingCutter) -> Vec<usize> {
    let lut = RadialProfileLUT::from_cutter(cutter, LUT_SAMPLES);
    let never_cancel = || false;
    let mut check = RapidClearanceCheck::new(cutter);
    stock
        .simulate_toolpath_with_lut_metrics_rapid_checked(
            tp,
            &lut,
            cutter,
            cutter.radius(),
            StockCutDirection::FromTop,
            ToolpathId(0),
            18_000,
            2,
            5000.0,
            0.25,
            None,
            &[],
            &[],
            true,
            &never_cancel,
            Some(&mut check),
        )
        .expect("never cancelled");
    check
        .into_hits()
        .into_iter()
        .map(|c| c.move_index)
        .collect()
}

/// The falsification the whole change rests on: a tightened descent must
/// never strike.
///
/// The optimizer's own answer — not a hand-picked Z — is replayed through the
/// independent live profile-aware rapid check (`RapidClearanceCheck`, riding
/// the simulator's walk so each rapid is judged against the stock AS IT
/// EXISTS at that point of playback). Zero hits.
///
/// Two guards keep that from passing vacuously:
///
/// * the optimization must have FIRED and must have moved the descent
///   strictly below the flat-disc answer — a no-op trivially cannot strike;
/// * the same check on the same stock with the same tool must FLAG a descent
///   that really does drive the flank into the crest, so a silent detector
///   cannot be mistaken for a safe path.
#[test]
fn the_tightened_descent_is_clear_under_the_live_check() {
    let tool = tapered_ball();
    let flat = flat_endmill();

    let stock = ridge_stock(RIDGE_OFFSET_MM);
    let (splits, tp) = optimize_one(&stock, QX, QY, &tool);
    assert_eq!(splits, 1, "non-vacuity: the optimizer must have fired");

    let taper_z = inserted_descent_z(&tp);
    let flat_z = inserted_descent_z(&optimize_one(&stock, QX, QY, &flat).1);
    assert!(
        taper_z < flat_z - 1e-9,
        "non-vacuity: the descent must actually have been tightened \
         ({taper_z:.6} vs the flat answer {flat_z:.6}); a no-op descent \
         cannot strike and would prove nothing"
    );

    let mut replay = ridge_stock(RIDGE_OFFSET_MM);
    assert_eq!(
        live_hits(&mut replay, &tp, &tool),
        Vec::<usize>::new(),
        "the profile-tightened descent to z = {taper_z:.6} struck material \
         under the live check. B2 may only lower a descent where the cutter's \
         own profile provably clears everything standing under it."
    );

    // The detector is awake on this fixture: a descent that genuinely drives
    // the flank into the crest IS flagged, by the same check, same stock,
    // same tool.
    let mut awake = ridge_stock(RIDGE_OFFSET_MM);
    let mut gouge = Toolpath::new();
    gouge.rapid_to_with_intent(
        P3::new(QX + RIDGE_OFFSET_MM, QY, SAFE_Z),
        MoveIntent::Linking,
    );
    gouge.rapid_to_with_intent(
        P3::new(QX + RIDGE_OFFSET_MM, QY, FLOOR_Z + 0.5),
        MoveIntent::Linking,
    );
    assert_eq!(
        live_hits(&mut awake, &gouge, &tool),
        vec![1],
        "vacuity guard: a rapid straight down into the crest must be flagged \
         by the same check — if it is silent, the zero above means nothing"
    );
}
