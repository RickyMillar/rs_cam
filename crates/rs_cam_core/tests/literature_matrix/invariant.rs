//! Invariant primitives: band-check, convex_hull, expr (closed-form), anti-pattern.
//!
//! Each primitive returns a [`SubVerdict`] tier (`Within` / `Edge` /
//! `Outside` / `NA`) plus a short reason string. The runner aggregates these
//! into a cell-level verdict.

use super::expr::{Bindings, evaluate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubVerdict {
    Within,
    Edge,
    Outside,
    NA,
}

impl SubVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            SubVerdict::Within => "Within",
            SubVerdict::Edge => "Edge",
            SubVerdict::Outside => "Outside",
            SubVerdict::NA => "NA",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubVerdictDetail {
    pub verdict: SubVerdict,
    pub reason: String,
}

impl SubVerdictDetail {
    pub fn within(reason: impl Into<String>) -> Self {
        Self {
            verdict: SubVerdict::Within,
            reason: reason.into(),
        }
    }
    pub fn edge(reason: impl Into<String>) -> Self {
        Self {
            verdict: SubVerdict::Edge,
            reason: reason.into(),
        }
    }
    pub fn outside(reason: impl Into<String>) -> Self {
        Self {
            verdict: SubVerdict::Outside,
            reason: reason.into(),
        }
    }
    pub fn na(reason: impl Into<String>) -> Self {
        Self {
            verdict: SubVerdict::NA,
            reason: reason.into(),
        }
    }
}

/// Mode parsed from the cell's TOML `mode = "..."`. The default is `Band`
/// when both min and max are present.
#[derive(Debug, Clone, Copy)]
pub enum BandMode {
    Band,
    Ceiling,
    Floor,
    Fraction,
    Na,
}

impl BandMode {
    pub fn parse(s: Option<&str>) -> Self {
        match s.unwrap_or("band").to_lowercase().as_str() {
            "band" => BandMode::Band,
            "ceiling" => BandMode::Ceiling,
            "floor" => BandMode::Floor,
            "fraction" => BandMode::Fraction,
            "na" => BandMode::Na,
            _ => BandMode::Band,
        }
    }
}

/// Band check on a single value. `edge_fraction` is the proximity to a band
/// edge that counts as `Edge` (plan default 0.10 = 10%).
pub fn band_check(
    value: f64,
    min: Option<f64>,
    max: Option<f64>,
    mode: BandMode,
    edge_fraction: f64,
) -> SubVerdictDetail {
    match mode {
        BandMode::Na => SubVerdictDetail::na("mode=na"),
        BandMode::Band => {
            let (Some(lo), Some(hi)) = (min, max) else {
                return SubVerdictDetail::na("band mode requires both min and max");
            };
            inside_band(value, lo, hi, edge_fraction)
        }
        BandMode::Ceiling => {
            let Some(hi) = max else {
                return SubVerdictDetail::na("ceiling mode requires max");
            };
            // value should be ≤ hi
            if value > hi {
                SubVerdictDetail::outside(format!(
                    "{value:.4} > ceiling {hi:.4} (+{:.1}%)",
                    100.0 * (value - hi) / hi.abs().max(f64::EPSILON)
                ))
            } else if value > hi * (1.0 - edge_fraction) {
                SubVerdictDetail::edge(format!(
                    "{value:.4} within {:.0}% of ceiling {hi:.4}",
                    edge_fraction * 100.0
                ))
            } else {
                SubVerdictDetail::within(format!("{value:.4} ≤ {hi:.4}"))
            }
        }
        BandMode::Floor => {
            let Some(lo) = min else {
                return SubVerdictDetail::na("floor mode requires min");
            };
            if value < lo {
                SubVerdictDetail::outside(format!(
                    "{value:.4} < floor {lo:.4} (-{:.1}%)",
                    100.0 * (lo - value) / lo.abs().max(f64::EPSILON)
                ))
            } else if value < lo * (1.0 + edge_fraction) {
                SubVerdictDetail::edge(format!(
                    "{value:.4} within {:.0}% of floor {lo:.4}",
                    edge_fraction * 100.0
                ))
            } else {
                SubVerdictDetail::within(format!("{value:.4} ≥ {lo:.4}"))
            }
        }
        BandMode::Fraction => {
            let (Some(lo), Some(hi)) = (min, max) else {
                return SubVerdictDetail::na("fraction mode requires min and max");
            };
            inside_band(value, lo, hi, edge_fraction)
        }
    }
}

fn inside_band(value: f64, lo: f64, hi: f64, edge_fraction: f64) -> SubVerdictDetail {
    if value < lo {
        SubVerdictDetail::outside(format!(
            "{value:.4} < min {lo:.4} (-{:.1}%)",
            100.0 * (lo - value) / lo.abs().max(f64::EPSILON)
        ))
    } else if value > hi {
        SubVerdictDetail::outside(format!(
            "{value:.4} > max {hi:.4} (+{:.1}%)",
            100.0 * (value - hi) / hi.abs().max(f64::EPSILON)
        ))
    } else {
        let span = (hi - lo).abs().max(f64::EPSILON);
        let lower_margin = (value - lo) / span;
        let upper_margin = (hi - value) / span;
        if lower_margin < edge_fraction || upper_margin < edge_fraction {
            SubVerdictDetail::edge(format!(
                "{value:.4} within {:.0}% of [{:.4}, {:.4}]",
                edge_fraction * 100.0,
                lo,
                hi
            ))
        } else {
            SubVerdictDetail::within(format!("{value:.4} ∈ [{:.4}, {:.4}]", lo, hi))
        }
    }
}

/// 2D point-in-polygon (ray-casting). Vertices may be given in any winding
/// order. Boundary points count as inside.
pub fn point_in_polygon(point: (f64, f64), polygon: &[[f64; 2]]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let (px, py) = point;
    let mut inside = false;
    let n = polygon.len();
    for i in 0..n {
        let (xi, yi) = (polygon[i][0], polygon[i][1]);
        let j = if i == 0 { n - 1 } else { i - 1 };
        let (xj, yj) = (polygon[j][0], polygon[j][1]);
        // boundary check: collinear & within segment bounding box
        let on_seg = ((py - yi).abs() < f64::EPSILON && (px - xi).abs() < f64::EPSILON)
            || on_segment(point, (xi, yi), (xj, yj));
        if on_seg {
            return true;
        }
        let crosses = (yi > py) != (yj > py);
        if crosses {
            let denom = yj - yi;
            // we already know yi != yj because crosses is true
            let x_intersect = xi + (py - yi) * (xj - xi) / denom;
            if px < x_intersect {
                inside = !inside;
            }
        }
    }
    inside
}

fn on_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> bool {
    let cross = (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
    if cross.abs() > 1e-9 {
        return false;
    }
    let min_x = a.0.min(b.0);
    let max_x = a.0.max(b.0);
    let min_y = a.1.min(b.1);
    let max_y = a.1.max(b.1);
    p.0 >= min_x - 1e-9 && p.0 <= max_x + 1e-9 && p.1 >= min_y - 1e-9 && p.1 <= max_y + 1e-9
}

/// Convex-hull / point-in-polygon invariant. Returns `Within` if the
/// 2D point is inside (or on) the polygon, `Outside` otherwise.
pub fn convex_hull_check(
    vars_xy: (f64, f64),
    vertices: &[[f64; 2]],
) -> SubVerdictDetail {
    if point_in_polygon(vars_xy, vertices) {
        SubVerdictDetail::within(format!(
            "({:.4}, {:.4}) inside envelope ({} vertices)",
            vars_xy.0,
            vars_xy.1,
            vertices.len()
        ))
    } else {
        SubVerdictDetail::outside(format!(
            "({:.4}, {:.4}) outside envelope",
            vars_xy.0, vars_xy.1
        ))
    }
}

/// Evaluate a closed-form expression. If `floor` / `ceiling` are given, the
/// numeric result is band-checked. Otherwise the result is treated as a
/// boolean (truthy = `Outside`, falsy = `Within`) for inequality-form
/// invariants like `doc / D <= 0.4`.
pub fn expr_check(
    src: &str,
    bindings: &Bindings,
    floor: Option<f64>,
    ceiling: Option<f64>,
    edge_fraction: f64,
) -> SubVerdictDetail {
    let v = match evaluate(src, bindings) {
        Ok(v) => v,
        Err(e) => return SubVerdictDetail::na(format!("expr error: {e}")),
    };
    match (floor, ceiling) {
        (Some(lo), Some(hi)) => band_check(v, Some(lo), Some(hi), BandMode::Band, edge_fraction),
        (Some(lo), None) => band_check(v, Some(lo), None, BandMode::Floor, edge_fraction),
        (None, Some(hi)) => band_check(v, None, Some(hi), BandMode::Ceiling, edge_fraction),
        (None, None) => {
            // boolean-form: any non-zero result means the inequality holds
            // (e.g. `doc / D <= 0.4` evaluates to 1.0 when satisfied).
            // For boolean invariants, the convention is: truthy = Within
            // (inequality holds), falsy = Outside.
            if v != 0.0 {
                SubVerdictDetail::within(format!("expr `{src}` = {v}"))
            } else {
                SubVerdictDetail::outside(format!("expr `{src}` = {v} (constraint violated)"))
            }
        }
    }
}

/// Anti-pattern: boolean expression that triggers `Outside` when truthy.
/// (Opposite polarity from `expr_check` in boolean mode — a truthy
/// anti-pattern means the *bad thing happened*.)
pub fn anti_pattern_check(src: &str, bindings: &Bindings) -> SubVerdictDetail {
    match evaluate(src, bindings) {
        Ok(v) if v != 0.0 => {
            SubVerdictDetail::outside(format!("anti-pattern `{src}` triggered (= {v})"))
        }
        Ok(_) => SubVerdictDetail::within(format!("anti-pattern `{src}` clear")),
        Err(e) => SubVerdictDetail::na(format!("anti-pattern expr error: {e}")),
    }
}
