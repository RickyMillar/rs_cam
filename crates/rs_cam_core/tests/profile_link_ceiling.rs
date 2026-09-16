//! Profile-aware link ceiling — the gouge guard for stay-down transits.
//!
//! ## The defect
//!
//! [`rs_cam_core::finish::surface_link::LinkCeiling`] decided how high a stay-down link
//! must fly by reading `max_conservative_top_z_in_disc` over a disc of the
//! tool's ENVELOPE radius. That models the cutter as a flat cylinder. It is
//! exact for a flat endmill and wrong for every other profile: past its tip a
//! real cutter RISES, so material at lateral offset `r` can only strike it if
//! it stands more than `height_at_radius(r)` above the tip.
//!
//! Measured on the operator's 200x200x9.81 mm relief with the shipped R1.0
//! tapered ball (ball 2.0 mm, 5.7 deg half-angle, 6 mm shank, envelope radius
//! 3.0 mm):
//!
//! ```text
//! offset r    tool height    can material there reach the tool?
//!   0.00mm         0.00mm    yes
//!   1.00mm         0.95mm    yes
//!   1.50mm         5.96mm    yes
//!   2.00mm        10.97mm    NO  - above the board's entire relief
//!   3.00mm        20.99mm    NO
//! ```
//!
//! Only material within ~1.5 mm laterally could physically touch that cutter on
//! that board; the ceiling was read over 3.0 mm — a 2x over-reach in radius —
//! and lifted every link to the height of ridges that cannot contact the tool.
//! Holding the path identical and varying only the ceiling height moved one
//! region from 898 s to 1411 s (1.57x).
//!
//! ## The rule
//!
//! ```text
//! required_tip_z(x,y) = max over r in [0, envelope_radius] of
//!                         [ material_top_on_annulus(x,y,r) - height_at_radius(r) ]
//! ```
//!
//! ## What is asserted here
//!
//! * GATE 1 — the SAFETY ANCHOR. For a flat endmill `height_at_radius(r)` is
//!   `Some(0.0)` throughout the envelope, so the rule reduces to
//!   `max over disc of material_top`: byte-identical to the old query, on the
//!   same stock, at the same radius. If this ever fails, the generalisation is
//!   not a generalisation.
//! * GATE 2 — a tapered ball does NOT lift for a ridge it cannot touch, and
//!   the relaxation is real (strictly below the flat-disc answer).
//! * GATE 3 — conservatism where it counts: a ridge under the tip produces
//!   exactly the old lift, and a ridge inside the BALL region can relax by at
//!   most the ball's own rise. The relaxation cannot reach material the tool
//!   can hit.
//! * GATE 4 — the physical no-contact invariant, recomputed cell by cell from
//!   the grid rather than from the query under test: at the returned tip Z, no
//!   dexel column anywhere in the disc pokes above the cutter's own profile.
//!   A relaxed ceiling cannot silently start gouging.
//!
//! ## Why it cannot pass vacuously
//!
//! GATE 2's non-vacuity half asserts the flat and profile answers DIFFER by
//! more than the ridge's own height above the floor; a stub returning the old
//! flat-disc answer fails it. GATE 1 fails any stub that always relaxes, and
//! GATE 4 fails any stub that relaxes too far.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::tool::{FlatEndmill, MillingCutter, TaperedBallEndmill};

// ── Fixture ─────────────────────────────────────────────────────────────

const CELL_MM: f64 = 0.25;
const STOCK_TOP_Z: f64 = 0.0;
const STOCK_BOTTOM_Z: f64 = -12.0;

/// Floor the terrain around the query point is cut down to.
const FLOOR_Z: f64 = -5.0;

/// Lateral offset of the ridge in [`ridge_stock`], in mm from the query point.
const RIDGE_OFFSET_MM: f64 = 2.5;

/// Half-width of that ridge band.
const RIDGE_HALF_WIDTH_MM: f64 = 0.3;

/// The query point every gate asks about. On a cell centre so the geometry is
/// easy to reason about by hand.
const QX: f64 = 0.0;
const QY: f64 = 0.0;

/// The shipped R1.0 tapered ball from the operator's board: ball 2.0 mm,
/// 5.7 deg half-angle, 6 mm shank. `envelope_radius_mm() == 3.0`.
fn tapered_ball() -> TaperedBallEndmill {
    TaperedBallEndmill::new(2.0, 5.7, 6.0, 25.0)
}

/// A 3 mm-radius flat endmill — the same envelope radius as
/// [`tapered_ball`], so GATE 1 and GATE 2 read the SAME disc and differ only
/// in the profile inside it.
fn flat_endmill() -> FlatEndmill {
    FlatEndmill::new(6.0, 25.0)
}

fn blank_stock() -> TriDexelStock {
    TriDexelStock::from_stock(
        -10.0,
        -10.0,
        10.0,
        10.0,
        STOCK_BOTTOM_Z,
        STOCK_TOP_Z,
        CELL_MM,
    )
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

/// Terrain cut down to [`FLOOR_Z`] everywhere except one narrow ridge band at
/// `y ≈ RIDGE_OFFSET_MM`, left standing at the raw stock top.
///
/// The ridge stands 5.0 mm above the floor. The tapered ball stands ~15.9 mm
/// above its own tip at that lateral offset, so the ridge CANNOT touch the
/// cutter when its tip is at the floor — which is exactly the case the flat
/// disc got wrong.
fn ridge_stock(offset_mm: f64) -> TriDexelStock {
    shaped_stock(&|_x, y| {
        if (y - offset_mm).abs() <= RIDGE_HALF_WIDTH_MM {
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
        (h - 3.0).clamp(STOCK_BOTTOM_Z + 0.5, STOCK_TOP_Z)
    })
}

/// Query points spread across the lumpy terrain, including two that hang off
/// the grid edge so the clamped-bbox arm is exercised too.
fn probe_points() -> Vec<(f64, f64)> {
    let mut pts = Vec::new();
    let mut x = -9.5;
    while x <= 9.5 {
        let mut y = -9.5;
        while y <= 9.5 {
            pts.push((x, y));
            y += 3.1;
        }
        x += 3.7;
    }
    pts.push((QX, QY));
    pts.push((-10.4, 0.13));
    pts.push((0.13, 10.4));
    pts
}

/// The physical invariant, recomputed from the raw grid: with the tip at
/// `tip_z`, no column in the disc may poke above the cutter's own profile.
///
/// Deliberately NOT the query under test — it walks every cell of the grid and
/// asks the cutter directly, so a wrong sign, a wrong search radius or a
/// skipped cell all surface here. The one thing it does restate rather than
/// re-derive is the disc convention (half-cell dilation on inclusion,
/// half-DIAGONAL on the profile lookup), because that convention IS the
/// contract, not an implementation detail.
///
/// Returns the worst penetration in mm (`<= 0` is clear; `== 0` means the
/// ceiling is tight, i.e. touching without cutting).
fn worst_penetration_mm(
    stock: &TriDexelStock,
    cx: f64,
    cy: f64,
    radius: f64,
    cutter: &dyn MillingCutter,
    tip_z: f64,
) -> f64 {
    let grid = &stock.z_grid;
    let (cs, ou, ov) = (grid.cell_size, grid.origin_u, grid.origin_v);
    // Inclusion: half a cell of dilation turns "centre inside the disc" into
    // "square overlaps the disc". Compared squared, exactly as the production
    // query does, so a cell cannot land on one side of the boundary here and
    // the other side there.
    let reach_sq = (radius + cs * 0.5).powi(2);
    let half_diag = cs * std::f64::consts::FRAC_1_SQRT_2;
    let mut worst = f64::NEG_INFINITY;
    for row in 0..grid.rows {
        let y = ov + row as f64 * cs;
        let dy = y - cy;
        for col in 0..grid.cols {
            let x = ou + col as f64 * cs;
            let dx = x - cx;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq > reach_sq {
                continue;
            }
            // Lookup: the cell's material may sit anywhere in its square, so
            // the closest it can be to the axis is the centre distance minus
            // the half-DIAGONAL. That lower bound picks the smallest tool
            // height the cell could see, hence the largest required lift.
            let d_near = (dist_sq.sqrt() - half_diag).max(0.0);
            let Some(h) = cutter.height_at_radius(d_near) else {
                continue;
            };
            let top = f64::from(grid.conservative_top_at(row, col));
            worst = worst.max(top - (tip_z + h));
        }
    }
    worst
}

// ── GATE 1: the safety anchor ───────────────────────────────────────────

/// A FLAT endmill's `height_at_radius` is `Some(0.0)` for every radius inside
/// its envelope, so `max over r of [top(r) - 0]` IS `max over disc of top`.
/// Byte-identical, not approximately equal: compared with `==` on `f64`.
///
/// This is the whole safety argument. The profile rule can only ever RELAX the
/// lift where the tool genuinely rises above its own tip — never for a flat
/// cutter, where it must not move at all.
#[test]
fn flat_endmill_profile_ceiling_is_byte_identical() {
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
    for (name, stock) in &stocks {
        for (x, y) in probe_points() {
            let old = stock.max_conservative_top_z_in_disc(x, y, radius);
            let new = stock.max_clearance_tip_z_for_profile(x, y, radius, &flat);
            assert_eq!(
                old, new,
                "{name} stock at ({x:.3}, {y:.3}): the profile-aware ceiling \
                 moved for a FLAT endmill. It must be byte-identical — \
                 height_at_radius is 0 everywhere inside a flat envelope, so \
                 there is nothing for the profile to relax."
            );
            compared += 1;
        }
    }
    assert!(
        compared > 50,
        "control: only {compared} points compared, the sweep collapsed"
    );
}

// ── GATE 2: a tapered ball clears a ridge it cannot touch ───────────────

/// The defect itself. A ridge 2.5 mm off-axis standing 5.0 mm above the floor
/// cannot reach a cutter that stands ~15.9 mm above its own tip out there, so
/// it must not raise the ceiling.
#[test]
fn a_tapered_ball_does_not_lift_for_a_ridge_it_cannot_touch() {
    let tool = tapered_ball();
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
        rise > STOCK_TOP_Z - FLOOR_Z,
        "fixture: the cutter stands {rise:.3} mm above its tip at the ridge's \
         inner edge, which must exceed the ridge's {:.3} mm of standing \
         material or this gate is not measuring the defect",
        STOCK_TOP_Z - FLOOR_Z
    );

    let flat_disc = stock
        .max_conservative_top_z_in_disc(QX, QY, radius)
        .expect("the query point is on-grid");
    let profile = stock
        .max_clearance_tip_z_for_profile(QX, QY, radius, &tool)
        .expect("the query point is on-grid");

    assert!(
        (flat_disc - STOCK_TOP_Z).abs() < 1e-9,
        "control: the flat disc must read the ridge top {STOCK_TOP_Z:.3}, got \
         {flat_disc:.3} — the fixture is not reproducing the over-reach"
    );
    assert!(
        (profile - FLOOR_Z).abs() < 1e-9,
        "the profile-aware ceiling must sit at the floor {FLOOR_Z:.3} (the only \
         material the tool can actually reach), got {profile:.3}"
    );
    assert!(
        flat_disc - profile >= STOCK_TOP_Z - FLOOR_Z - 1e-9,
        "non-vacuity: the relaxation must be the ridge's whole height, \
         {:.3} mm; measured {:.3} mm",
        STOCK_TOP_Z - FLOOR_Z,
        flat_disc - profile
    );

    // ...and it still clears everything the tool CAN reach.
    let worst = worst_penetration_mm(&stock, QX, QY, radius, &tool, profile);
    assert!(
        worst <= 1e-9,
        "the relaxed ceiling let the cutter into {worst:.6} mm of material"
    );
}

// ── GATE 3: conservatism is preserved ───────────────────────────────────

/// Material the tool CAN hit must still be cleared. Two arms:
///
/// * directly under the tip (`r = 0`, where `height_at_radius` is 0 for every
///   profile) the answer must be EXACTLY the old one;
/// * inside the ball region the answer may relax, but by at most the ball's
///   own rise — never by the shank's. That bound is the statement "the
///   relaxation cannot reach material the tool can hit".
#[test]
fn a_ridge_the_tool_can_strike_still_lifts_it() {
    let tool = tapered_ball();
    let radius = tool.envelope_radius_mm();
    let ball_radius = tool.ball_diameter / 2.0;

    // Arm A — ridge straight under the tip.
    let under_tip = ridge_stock(0.0);
    let flat_a = under_tip
        .max_conservative_top_z_in_disc(QX, QY, radius)
        .expect("on-grid");
    let profile_a = under_tip
        .max_clearance_tip_z_for_profile(QX, QY, radius, &tool)
        .expect("on-grid");
    assert_eq!(
        flat_a, profile_a,
        "material under the TIP is at r = 0, where every profile has height 0: \
         the ceiling must not move by a single ULP"
    );

    // Arm B — ridge inside the ball region.
    let inner_offset = ball_radius * 0.5;
    assert!(
        inner_offset + RIDGE_HALF_WIDTH_MM < ball_radius,
        "fixture: the inner ridge must lie wholly inside the ball region"
    );
    let inner = ridge_stock(inner_offset);
    let flat_b = inner
        .max_conservative_top_z_in_disc(QX, QY, radius)
        .expect("on-grid");
    let profile_b = inner
        .max_clearance_tip_z_for_profile(QX, QY, radius, &tool)
        .expect("on-grid");
    let ball_rise = tool
        .height_at_radius(ball_radius)
        .expect("the ball edge is inside the envelope");
    assert!(
        profile_b <= flat_b + 1e-12,
        "the profile rule may only ever relax: {profile_b:.6} > {flat_b:.6}"
    );
    assert!(
        flat_b - profile_b <= ball_rise + 1e-9,
        "a ridge the BALL can strike relaxed by {:.6} mm, more than the ball's \
         own {ball_rise:.6} mm rise — the relaxation reached material the tool \
         can hit",
        flat_b - profile_b
    );
    // And it is nowhere near the shank relaxation GATE 2 measured, which is
    // what makes "close enough to strike" a different regime and not just a
    // smaller number.
    assert!(
        flat_b - profile_b < (STOCK_TOP_Z - FLOOR_Z) * 0.25,
        "an in-ball ridge relaxed by {:.3} mm, comparable to the out-of-reach \
         case — the two regimes must not blur",
        flat_b - profile_b
    );

    let worst = worst_penetration_mm(&inner, QX, QY, radius, &tool, profile_b);
    assert!(
        worst <= 1e-9,
        "the in-ball ceiling let the cutter into {worst:.6} mm of material"
    );
}

// ── GATE 4: no ceiling may pass through material ────────────────────────

/// The invariant swept over the whole fixture set and both profiles: at the
/// ceiling the query returns, nothing anywhere in the search disc pokes above
/// the cutter. Recomputed from the raw grid with the true cell distances, so a
/// wrong radius, a wrong sign, or a skipped cell all surface here.
#[test]
fn no_profile_ceiling_passes_through_material() {
    let tapered = tapered_ball();
    let flat = flat_endmill();
    let ball = rs_cam_core::tool::BallEndmill::new(2.0, 25.0);
    let cutters: [(&str, &dyn MillingCutter); 3] =
        [("tapered", &tapered), ("flat", &flat), ("ball", &ball)];

    let stocks = [
        ("lumpy", lumpy_stock()),
        ("ridge_far", ridge_stock(RIDGE_OFFSET_MM)),
        ("ridge_near", ridge_stock(0.4)),
        ("ridge_tip", ridge_stock(0.0)),
    ];

    let mut checked = 0usize;
    let mut tightest = f64::NEG_INFINITY;
    for (cname, cutter) in cutters {
        let radius = cutter.envelope_radius_mm();
        for (sname, stock) in &stocks {
            for (x, y) in probe_points() {
                let ceiling = stock.max_clearance_tip_z_for_profile(x, y, radius, cutter);
                let Some(tip_z) = ceiling else {
                    // Off-grid: the caller falls back to the analytic top.
                    continue;
                };
                let worst = worst_penetration_mm(stock, x, y, radius, cutter, tip_z);
                assert!(
                    worst <= 1e-9,
                    "{cname} on {sname} at ({x:.3}, {y:.3}): ceiling {tip_z:.6} \
                     leaves {worst:.6} mm of material inside the cutter"
                );
                tightest = tightest.max(worst);
                checked += 1;
            }
        }
    }
    assert!(
        checked > 200,
        "control: only {checked} ceilings checked, the sweep collapsed"
    );
    // Non-vacuity: a ceiling of +infinity would clear everything and prove
    // nothing. The rule is a MAX, so on ground that holds material the tool
    // must end up exactly touching — worst == 0 — somewhere in the sweep.
    assert!(
        tightest.abs() <= 1e-9,
        "the ceiling is never tight anywhere in the sweep (best {tightest:.9} mm): \
         it is not the max of [top - height_at_radius], it is something looser"
    );
}
