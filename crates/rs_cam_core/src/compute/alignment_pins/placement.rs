//! The keyed-pin placer: one pin on the flip axis plus a mirror pair, in
//! the clear area of the stock (P6, 2026-10-12).

use crate::compute::transform::{FaceUp, ZRotation};

use super::keying::closest_wrong_seating;
use super::pattern::{AxisFrame, AxisPairPattern, axis_plus_mirror_pair};
use super::walls::{PinObstacle, clear_runs, pin_walls};
use super::{AlignmentPin, PIN_GEOM_EPS_MM, PIN_MATCH_TOL_MM, PinFlipKind, PinSetupMap};

/// Everything [`plan_keyed_pins`](crate::compute::stock_config::StockConfig::plan_keyed_pins)
/// needs, in **stock-local** millimetres (X0Y0 at the stock's min corner —
/// the frame `StockConfig::alignment_pins` is dimensioned in).
#[derive(Debug, Clone, PartialEq)]
pub struct PinPlacementRequest {
    /// Stock X extent.
    pub stock_w: f64,
    /// Stock Y extent.
    pub stock_d: f64,
    /// The 2D outlines a pin must keep a wall to: the part footprint, the
    /// keep-out zones and the fixtures, in stock-local mm. An empty list
    /// means that the whole blank is clear.
    ///
    /// `padding` is the wrong source for the part. A blank sized 140x150 by
    /// hand around a 100x100 model has a 20 mm clear strip while `padding`
    /// still reads 5.
    pub obstacles: Vec<PinObstacle>,
    /// The face the flipped setup puts up.
    pub face_up: FaceUp,
    /// The turn of the flipped setup. With `face_up`, it sets the flip
    /// axis the pins must register.
    pub z_rotation: ZRotation,
    /// Pin diameter: the dowel, and therefore the hole, and therefore
    /// the drill.
    pub pin_diameter: f64,
    /// Clearance from the pin's edge to the stock edge, to each obstacle
    /// and to the other pins.
    pub wall_mm: f64,
}

/// Why no keyed pin pattern exists for this blank.
///
/// Every variant is a refusal, not a warning: a placement that hangs off
/// the stock or that seats turned is worse than no placement, because the
/// operator finds out at the flip.
#[derive(Debug, Clone, PartialEq)]
pub enum PinPlacementError {
    /// The setup has no flip axis in the pin plane: a `Top` setup, an
    /// edge-up setup, or a quarter turn on a stock that is not square.
    UnsupportedFlip {
        face_up: FaceUp,
        z_rotation: ZRotation,
    },
    /// Stock or pin dimensions that no placement can be defined against.
    DegenerateStock {
        stock_w: f64,
        stock_d: f64,
        pin_diameter: f64,
    },
    /// No point of the flip axis can hold a pin with its walls. This is
    /// the live wanaka case: a 5 mm ring, a 6 mm dowel.
    NoRoomOnAxis {
        /// The axis as the operator reads it, for example `y = 75.0 mm`.
        axis: String,
        /// The pin diameter plus a wall each side.
        need_mm: f64,
        pin_diameter_mm: f64,
    },
    /// The stock is not wide enough across the flip axis for a mirror
    /// pair: two pins, a wall between them, and a wall to each edge.
    PairDoesNotFit {
        across_mm: f64,
        required_mm: f64,
        pin_diameter_mm: f64,
    },
    /// The stock is wide enough for a pair, but no pair position is clear
    /// of the part, the keep-outs and the fixtures.
    NoRoomForPair { need_mm: f64, pin_diameter_mm: f64 },
    /// Every pattern that fits seats turned.
    CannotKey { best_miss_mm: f64, required_mm: f64 },
}

impl std::fmt::Display for PinPlacementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PinPlacementError::UnsupportedFlip {
                face_up: FaceUp::Bottom,
                z_rotation,
            } => write!(
                f,
                "Cannot place registration pins for a Bottom setup turned {}: on a stock that \
                 is not square that turn changes the footprint, so the flip has no axis in the \
                 pin plane. Turn the flipped setup 0 or 180 deg.",
                z_rotation.label()
            ),
            PinPlacementError::UnsupportedFlip { face_up, .. } => write!(
                f,
                "Cannot place registration pins for a '{}' setup: only a Bottom flip keeps the \
                 part's XY footprint, the other orientations change which stock dimensions the \
                 pins would have to register",
                face_up.label()
            ),
            PinPlacementError::DegenerateStock {
                stock_w,
                stock_d,
                pin_diameter,
            } => write!(
                f,
                "Cannot place registration pins: stock {stock_w:.1}x{stock_d:.1} mm with a \
                 {pin_diameter:.1} mm pin is not a placeable geometry"
            ),
            PinPlacementError::NoRoomOnAxis {
                axis,
                need_mm,
                pin_diameter_mm,
            } => write!(
                f,
                "Cannot place registration pins: no point of the flip axis ({axis}) is clear. A \
                 {pin_diameter_mm:.1} mm pin needs {need_mm:.1} mm of material outside the part, \
                 the keep-outs and the fixtures (the pin plus a wall each side). Widen the \
                 stock, choose fewer part models, or fit a smaller dowel and drill."
            ),
            PinPlacementError::PairDoesNotFit {
                across_mm,
                required_mm,
                pin_diameter_mm,
            } => write!(
                f,
                "Cannot place registration pins: the stock is {across_mm:.1} mm across the flip \
                 axis, but a mirror pair of {pin_diameter_mm:.1} mm pins needs {required_mm:.1} \
                 mm (two pins, a wall between them and a wall to each edge). Enlarge the stock, \
                 or fit a smaller dowel and drill."
            ),
            PinPlacementError::NoRoomForPair {
                need_mm,
                pin_diameter_mm,
            } => write!(
                f,
                "Cannot place registration pins: no mirror pair across the flip axis is clear. \
                 Each {pin_diameter_mm:.1} mm pin needs {need_mm:.1} mm of material outside the \
                 part, the keep-outs and the fixtures (the pin plus a wall each side). Widen the \
                 stock, choose fewer part models, or fit a smaller dowel and drill."
            ),
            PinPlacementError::CannotKey {
                best_miss_mm,
                required_mm,
            } => write!(
                f,
                "Cannot place registration pins: every pin pattern that fits also seats turned. \
                 The best one misses a wrong seating by only {best_miss_mm:.1} mm, and \
                 {required_mm:.1} mm is needed. Widen the stock, or fit a smaller dowel."
            ),
        }
    }
}

/// One candidate position of a pin role, in the axis frame.
#[derive(Debug, Clone, Copy)]
struct Spot {
    along: f64,
    /// The pair offset from the axis; zero for the axis pin.
    across: f64,
    /// The smallest wall of the pin (both pins, for a pair).
    wall_mm: f64,
}

/// The start, middle and end of each run, each with `true` when it is the
/// middle of a run longer than a point.
///
/// A run ends where a wall is exactly the need, so an end needs no wall
/// evaluation: its margin is zero.
fn run_points(runs: &[(f64, f64)]) -> Vec<(f64, bool)> {
    let mut out = Vec::new();
    for &(t0, t1) in runs {
        out.push((t0, false));
        if t1 - t0 > PIN_GEOM_EPS_MM {
            out.push((0.5 * (t0 + t1), true));
            out.push((t1, false));
        }
    }
    out
}

/// The overlaps of two sorted run lists.
fn intersect_runs(a: &[(f64, f64)], b: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for &(a0, a1) in a {
        for &(b0, b1) in b {
            let (t0, t1) = (a0.max(b0), a1.min(b1));
            if t0 <= t1 + PIN_GEOM_EPS_MM {
                out.push((t0, t1.max(t0)));
            }
        }
    }
    out
}

/// Place a keyed pattern: one pin on the flip axis and a mirror pair off
/// the axis, in the clear area of the stock.
///
/// Two pins of one diameter never key: a half turn about their midpoint
/// puts each hole on the other dowel. One pin on the axis plus a mirror
/// pair is the smallest pattern of equal pins that seats under the flip
/// and keys, unless the three pins are in a line or form an equilateral
/// triangle.
///
/// The search works in the axis frame of the flip
/// ([`PinSetupMap::axis_segment`]): `along` the axis and `across` it. So
/// one search serves a `Bottom` setup at 0 deg (axis `y = D/2`), at
/// 180 deg (axis `x = W/2`) and a quarter turn on a square stock (a
/// diagonal).
///
/// 1. Refuse a setup with no flip axis in the pin plane.
/// 2. A point is clear when a pin there has a wall of `wall_mm` to each
///    stock edge and to each obstacle ([`clear_runs`] gives the clear runs
///    of a line exactly, from the outlines).
/// 3. The axis pin: the clear runs of the axis. The candidates are the
///    start, the middle and the end of each run.
/// 4. The pair: for each offset `h` from the largest that keeps a wall to
///    the stock edges down to the smallest that keeps a wall between the
///    two pair pins, in steps of [`PIN_MATCH_TOL_MM`] (a finer step moves a
///    hole by less than the hole slop), take the runs where the lines at
///    `+h` and `-h` are both clear. The candidates are the start, the
///    middle and the end of each run.
/// 5. Rank each pattern of an axis candidate and a pair candidate whose
///    pins keep a wall between them:
///    - first, the keying tier. The first choice is a pattern whose every
///      wrong seating misses by a full pin diameter (a dowel then does not
///      overlap any hole). The second choice is a pattern keyed at the bar
///      ([`keying_bar_mm`](super::keying_bar_mm));
///    - then the smallest wall of the three pins, in steps of
///      [`PIN_MATCH_TOL_MM`] above `wall_mm` (the margin). The middle of a
///      run is the point of the largest wall across a strip or a rim;
///    - then the shortest side of the triangle (a wider pattern holds the
///      part more firmly and turns it less for a given hole slop);
///    - then the larger `h`, then the larger `along` of the axis pin. On
///      a `Bottom` setup at 0 deg the last rule puts the axis pin in the +X
///      strip on a tie.
///
/// The result is `[axis pin, pair pin at -h, pair pin at +h]` (for a
/// `Bottom` setup at 0 deg: axis, low Y, high Y).
pub fn place_keyed_pins(req: &PinPlacementRequest) -> Result<[AlignmentPin; 3], PinPlacementError> {
    let map = PinSetupMap::new(req.face_up, req.z_rotation, req.stock_w, req.stock_d);
    let unsupported = PinPlacementError::UnsupportedFlip {
        face_up: req.face_up,
        z_rotation: req.z_rotation,
    };
    if map.kind() != PinFlipKind::InPlane {
        return Err(unsupported);
    }
    if !req.stock_w.is_finite()
        || !req.stock_d.is_finite()
        || !req.pin_diameter.is_finite()
        || req.stock_w <= 0.0
        || req.stock_d <= 0.0
        || req.pin_diameter <= 0.0
    {
        return Err(PinPlacementError::DegenerateStock {
            stock_w: req.stock_w,
            stock_d: req.stock_d,
            pin_diameter: req.pin_diameter,
        });
    }
    let Some(frame) = AxisFrame::of(&map) else {
        return Err(unsupported);
    };

    let d = req.pin_diameter;
    let r = 0.5 * d;
    let wall = req.wall_mm;
    let step = PIN_MATCH_TOL_MM;
    // The line `across = c` as an origin and a unit direction.
    let line = |c: f64| {
        let o = frame.point(0.0, c);
        let p = frame.point(1.0, c);
        (o, (p.0 - o.0, p.1 - o.1))
    };
    let runs_at = |c: f64| {
        let (o, u) = line(c);
        clear_runs(req.stock_w, req.stock_d, &req.obstacles, o, u, r, wall)
    };
    let wall_at = |along: f64, across: f64| {
        let (x, y) = frame.point(along, across);
        pin_walls(
            &AlignmentPin::new(x, y, d),
            req.stock_w,
            req.stock_d,
            &req.obstacles,
        )
        .min_mm()
    };

    // 3. The axis pin.
    let axis_spots: Vec<Spot> = run_points(&runs_at(0.0))
        .into_iter()
        .map(|(along, middle)| Spot {
            along,
            across: 0.0,
            wall_mm: if middle { wall_at(along, 0.0) } else { wall },
        })
        .collect();
    if axis_spots.is_empty() {
        return Err(PinPlacementError::NoRoomOnAxis {
            axis: map.axis_label().unwrap_or_default(),
            need_mm: d + 2.0 * wall,
            pin_diameter_mm: d,
        });
    }

    // 4. The pair. The flip maps the stock onto itself, so the stock
    // reaches equally far to each side of the axis.
    let reach = [
        (0.0, 0.0),
        (req.stock_w, 0.0),
        (req.stock_w, req.stock_d),
        (0.0, req.stock_d),
    ]
    .into_iter()
    .map(|(x, y)| frame.coords(x, y).1.abs())
    .fold(0.0_f64, f64::max);
    let h_min = 0.5 * (d + wall);
    let h_max = reach - r - wall;
    if h_max < h_min - PIN_GEOM_EPS_MM {
        return Err(PinPlacementError::PairDoesNotFit {
            across_mm: 2.0 * reach,
            required_mm: 2.0 * d + 3.0 * wall,
            pin_diameter_mm: d,
        });
    }
    let mut pair_spots: Vec<Spot> = Vec::new();
    let mut h = h_max.max(h_min);
    loop {
        let runs = intersect_runs(&runs_at(h), &runs_at(-h));
        for (along, middle) in run_points(&runs) {
            pair_spots.push(Spot {
                along,
                across: h,
                wall_mm: if middle {
                    wall_at(along, h).min(wall_at(along, -h))
                } else {
                    wall
                },
            });
        }
        if h <= h_min + PIN_GEOM_EPS_MM {
            break;
        }
        h = (h - step).max(h_min);
    }
    if pair_spots.is_empty() {
        return Err(PinPlacementError::NoRoomForPair {
            need_mm: d + 2.0 * wall,
            pin_diameter_mm: d,
        });
    }

    // 5. Rank. The keying check is the costly part, so the patterns are
    // sorted by the geometric rules first and checked in that order.
    struct Scored {
        margin_steps: i64,
        spread: f64,
        h: f64,
        axis_along: f64,
        pins: [AlignmentPin; 3],
    }
    let mut scored: Vec<Scored> = Vec::new();
    for a in &axis_spots {
        for p in &pair_spots {
            let Some(pins) = axis_plus_mirror_pair(
                &map,
                AxisPairPattern {
                    axis_along: a.along,
                    pair_along: p.along,
                    pair_across: p.across,
                    diameter: d,
                },
            ) else {
                continue;
            };
            let [axis, low, high] = &pins;
            let side = |u: &AlignmentPin, v: &AlignmentPin| {
                (u.at.x() - v.at.x()).hypot(u.at.y() - v.at.y())
            };
            let to_pair = side(axis, low).min(side(axis, high));
            if to_pair < d + wall - PIN_GEOM_EPS_MM {
                continue;
            }
            let margin = a.wall_mm.min(p.wall_mm) - wall;
            // SAFETY: a margin in steps of 0.5 mm on a stock of finite size
            // fits an i64 with room to spare.
            #[allow(clippy::cast_possible_truncation)]
            let margin_steps = (margin / step + 1e-6).floor().max(0.0) as i64;
            scored.push(Scored {
                margin_steps,
                spread: to_pair.min(side(low, high)),
                h: p.across,
                axis_along: a.along,
                pins,
            });
        }
    }
    scored.sort_by(|a, b| {
        b.margin_steps
            .cmp(&a.margin_steps)
            .then(b.spread.total_cmp(&a.spread))
            .then(b.h.total_cmp(&a.h))
            .then(b.axis_along.total_cmp(&a.axis_along))
    });

    let bar = 0.5 * d;
    let mut best_miss: f64 = 0.0;
    let mut keyed_at_bar: Option<[AlignmentPin; 3]> = None;
    for candidate in scored {
        let miss = closest_wrong_seating(&candidate.pins).map(|w| w.miss_mm);
        // No turn seats at all: keyed in the first tier.
        if miss.is_none_or(|m| m >= d - PIN_GEOM_EPS_MM) {
            return Ok(candidate.pins);
        }
        let m = miss.unwrap_or(0.0);
        best_miss = best_miss.max(m);
        if keyed_at_bar.is_none() && m >= bar - PIN_GEOM_EPS_MM {
            keyed_at_bar = Some(candidate.pins);
        }
    }
    keyed_at_bar.ok_or(PinPlacementError::CannotKey {
        best_miss_mm: best_miss,
        // The bar of a pattern of equal pins is their radius.
        required_mm: bar,
    })
}
