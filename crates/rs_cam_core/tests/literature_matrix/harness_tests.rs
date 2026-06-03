//! Golden-input → golden-verdict tests for the harness primitives.
//!
//! These are independent of any actual cell content; they exercise each
//! primitive (band-check, convex_hull, expr evaluator, anti-pattern) on
//! hand-crafted inputs so the harness can't silently rot.

use super::expr::{Bindings, evaluate};
use super::invariant::{
    BandMode, SubVerdict, anti_pattern_check, band_check, convex_hull_check, expr_check,
    point_in_polygon,
};
use super::verdict::{CellVerdict, Severity, SubVerdictRow, rollup};

fn bindings(pairs: &[(&str, f64)]) -> Bindings {
    let mut b = Bindings::new();
    for (k, v) in pairs {
        b.insert((*k).into(), *v);
    }
    b
}

#[test]
fn band_check_inside_band_is_within() {
    let d = band_check(1.5, Some(1.0), Some(2.0), BandMode::Band, 0.10);
    assert_eq!(d.verdict, SubVerdict::Within, "{}", d.reason);
}

#[test]
fn band_check_near_edge_is_edge_tier() {
    // value 1.05 in [1.0, 2.0] -> lower margin 5% < 10% threshold
    let d = band_check(1.05, Some(1.0), Some(2.0), BandMode::Band, 0.10);
    assert_eq!(d.verdict, SubVerdict::Edge, "{}", d.reason);

    // and on the upper edge
    let d2 = band_check(1.95, Some(1.0), Some(2.0), BandMode::Band, 0.10);
    assert_eq!(d2.verdict, SubVerdict::Edge, "{}", d2.reason);
}

#[test]
fn band_check_outside_is_outside() {
    let lo = band_check(0.5, Some(1.0), Some(2.0), BandMode::Band, 0.10);
    assert_eq!(lo.verdict, SubVerdict::Outside);
    let hi = band_check(2.5, Some(1.0), Some(2.0), BandMode::Band, 0.10);
    assert_eq!(hi.verdict, SubVerdict::Outside);
}

#[test]
fn band_check_ceiling_floor_modes() {
    // ceiling: value <= max
    let c_in = band_check(1.5, None, Some(2.0), BandMode::Ceiling, 0.10);
    assert_eq!(c_in.verdict, SubVerdict::Within);
    let c_out = band_check(2.5, None, Some(2.0), BandMode::Ceiling, 0.10);
    assert_eq!(c_out.verdict, SubVerdict::Outside);
    // floor: value >= min
    let f_in = band_check(2.0, Some(1.0), None, BandMode::Floor, 0.10);
    assert_eq!(f_in.verdict, SubVerdict::Within);
    let f_out = band_check(0.5, Some(1.0), None, BandMode::Floor, 0.10);
    assert_eq!(f_out.verdict, SubVerdict::Outside);
}

/// Round-3 fix: when the engine deliberately clamps and its output
/// lands exactly at the floor/ceiling/band-edge, that's success — not
/// an Edge "almost-failed" signal. Regression net for the
/// drill_rpm_ceiling, rubbing-floor chipload, and band-edge RPM
/// false-alarms in literature_matrix Phase 1.
#[test]
fn band_check_at_clamp_is_within_not_edge() {
    // Ceiling-mode: engine clamped RPM to 14_000 (drill rpm ceiling).
    let drill_rpm = band_check(14_000.0, None, Some(14_000.0), BandMode::Ceiling, 0.10);
    assert_eq!(drill_rpm.verdict, SubVerdict::Within, "{}", drill_rpm.reason);
    // Approaching the ceiling but not at it — still Edge.
    let near_ceiling = band_check(13_500.0, None, Some(14_000.0), BandMode::Ceiling, 0.10);
    assert_eq!(near_ceiling.verdict, SubVerdict::Edge, "{}", near_ceiling.reason);

    // Floor-mode: engine clamped chipload to 0.025 mm/tooth (rubbing floor).
    let rubbing = band_check(0.025, Some(0.025), None, BandMode::Floor, 0.10);
    assert_eq!(rubbing.verdict, SubVerdict::Within, "{}", rubbing.reason);
    // Slightly above floor — still Edge.
    let near_floor = band_check(0.026, Some(0.025), None, BandMode::Floor, 0.10);
    assert_eq!(near_floor.verdict, SubVerdict::Edge, "{}", near_floor.reason);

    // Band-mode: engine RPM 20_000 inside literature band [16_000, 20_000]
    // (vendor LUT max coincides with band top → at-edge is success).
    let band_top = band_check(20_000.0, Some(16_000.0), Some(20_000.0), BandMode::Band, 0.10);
    assert_eq!(band_top.verdict, SubVerdict::Within, "{}", band_top.reason);
    let band_bot = band_check(16_000.0, Some(16_000.0), Some(20_000.0), BandMode::Band, 0.10);
    assert_eq!(band_bot.verdict, SubVerdict::Within, "{}", band_bot.reason);
    // 1% inside the upper edge — still Edge.
    let inside_upper = band_check(19_800.0, Some(16_000.0), Some(20_000.0), BandMode::Band, 0.10);
    assert_eq!(inside_upper.verdict, SubVerdict::Edge, "{}", inside_upper.reason);
}

#[test]
fn point_in_polygon_square_basic() {
    let square = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    assert!(point_in_polygon((0.5, 0.5), &square));
    assert!(!point_in_polygon((1.5, 0.5), &square));
    assert!(!point_in_polygon((-0.1, 0.5), &square));
    // boundary
    assert!(point_in_polygon((0.0, 0.5), &square));
}

#[test]
fn convex_hull_check_pocket_envelope() {
    // envelope_pocket vertices from the worked example
    let env = [[0.40, 0.25], [0.75, 0.25], [0.75, 0.75], [0.40, 0.75]];
    let inside = convex_hull_check((0.5, 0.5), &env);
    assert_eq!(inside.verdict, SubVerdict::Within, "{}", inside.reason);
    let outside = convex_hull_check((0.2, 0.5), &env);
    assert_eq!(outside.verdict, SubVerdict::Outside, "{}", outside.reason);
}

#[test]
fn expr_evaluator_arithmetic_and_precedence() {
    let b = bindings(&[]);
    assert_eq!(evaluate("2 + 3 * 4", &b).unwrap(), 14.0);
    assert_eq!(evaluate("(2 + 3) * 4", &b).unwrap(), 20.0);
    assert_eq!(evaluate("-3 + 1", &b).unwrap(), -2.0);
    assert_eq!(evaluate("10 / 4", &b).unwrap(), 2.5);
}

#[test]
fn expr_evaluator_identifiers_and_div_by_zero() {
    let b = bindings(&[("rpm", 18000.0), ("flutes", 2.0), ("feed_rate", 720.0)]);
    // feed / (rpm * flutes) = 720 / 36000 = 0.02
    let v = evaluate("feed_rate / (rpm * flutes)", &b).unwrap();
    assert!((v - 0.02).abs() < 1e-12);

    // div-by-zero
    let z = bindings(&[("a", 1.0), ("b", 0.0)]);
    assert!(matches!(
        evaluate("a / b", &z),
        Err(super::expr::EvalError::DivByZero)
    ));

    // unknown var
    assert!(matches!(
        evaluate("missing_var * 2", &b),
        Err(super::expr::EvalError::UnknownVar(_))
    ));
}

#[test]
fn expr_evaluator_comparisons_and_logic() {
    let b = bindings(&[("x", 5.0), ("y", 10.0)]);
    assert_eq!(evaluate("x < y", &b).unwrap(), 1.0);
    assert_eq!(evaluate("x > y", &b).unwrap(), 0.0);
    assert_eq!(evaluate("x == 5", &b).unwrap(), 1.0);
    assert_eq!(evaluate("x != 5", &b).unwrap(), 0.0);
    assert_eq!(evaluate("x < y && y > 0", &b).unwrap(), 1.0);
    assert_eq!(evaluate("x > y || y == 10", &b).unwrap(), 1.0);
}

#[test]
fn expr_check_floor_form() {
    // chipload above 0.035 rubbing floor
    let b = bindings(&[("rpm", 18000.0), ("flutes", 2.0), ("feed_rate", 2520.0)]);
    // feed / (rpm * flutes) = 0.07
    let d = expr_check("feed_rate / (rpm * flutes)", &b, Some(0.035), None, 0.10);
    assert_eq!(d.verdict, SubVerdict::Within, "{}", d.reason);

    // below floor
    let b_low = bindings(&[("rpm", 18000.0), ("flutes", 2.0), ("feed_rate", 720.0)]);
    let d_low = expr_check(
        "feed_rate / (rpm * flutes)",
        &b_low,
        Some(0.035),
        None,
        0.10,
    );
    assert_eq!(d_low.verdict, SubVerdict::Outside, "{}", d_low.reason);
}

#[test]
fn anti_pattern_triggers_outside_when_truthy() {
    let b = bindings(&[("rpm", 18000.0), ("flutes", 2.0), ("feed_rate", 540.0)]);
    // 540 / (18000 * 2) = 0.015 < 0.025
    let d = anti_pattern_check("feed_rate / (rpm * flutes) < 0.025", &b);
    assert_eq!(d.verdict, SubVerdict::Outside, "{}", d.reason);

    let b_clear = bindings(&[("rpm", 18000.0), ("flutes", 2.0), ("feed_rate", 2520.0)]);
    let d_clear = anti_pattern_check("feed_rate / (rpm * flutes) < 0.025", &b_clear);
    assert_eq!(d_clear.verdict, SubVerdict::Within, "{}", d_clear.reason);
}

#[test]
fn rollup_promotes_outside_to_declared_severity() {
    let rows = vec![
        SubVerdictRow {
            label: "rpm".into(),
            detail: super::invariant::SubVerdictDetail::within("ok"),
            severity_on_fail: Severity::Moderate,
        },
        SubVerdictRow {
            label: "anti.rubbing".into(),
            detail: super::invariant::SubVerdictDetail::outside("rubbing"),
            severity_on_fail: Severity::Critical,
        },
    ];
    let v: CellVerdict = rollup("test", rows);
    assert_eq!(v.overall, Severity::Critical);
    assert!(v.overall.blocks_ci());
}

#[test]
fn rollup_edge_drops_one_severity_tier() {
    let rows = vec![SubVerdictRow {
        label: "fpt".into(),
        detail: super::invariant::SubVerdictDetail::edge("near edge"),
        severity_on_fail: Severity::Critical,
    }];
    let v = rollup("test_edge", rows);
    // Edge on a critical-on-fail row contributes Major, not Critical.
    assert_eq!(v.overall, Severity::Major);
}

#[test]
fn rollup_all_within_is_cosmetic() {
    let rows = vec![SubVerdictRow {
        label: "rpm".into(),
        detail: super::invariant::SubVerdictDetail::within("ok"),
        severity_on_fail: Severity::Critical,
    }];
    let v = rollup("test_clean", rows);
    assert_eq!(v.overall, Severity::Cosmetic);
    assert!(!v.overall.blocks_ci());
}
