//! The keyed-pin placer: one pin on the flip axis plus a mirror pair.

use crate::compute::transform::{FaceUp, ZRotation};

use super::keying::{closest_wrong_seating, pattern_is_keyed};
use super::{AlignmentPin, PIN_GEOM_EPS_MM, PIN_MATCH_TOL_MM};

/// Everything [`plan_keyed_pins`](crate::compute::stock_config::StockConfig::plan_keyed_pins)
/// needs, in **stock-local** millimetres (X0Y0 at the stock's min corner —
/// the frame `StockConfig::alignment_pins` is dimensioned in).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PinPlacementRequest {
    /// Stock X extent.
    pub stock_w: f64,
    /// Stock Y extent.
    pub stock_d: f64,
    /// The model's X span in stock-local mm — the strip a pin may NOT
    /// enter. `None` when no model is loaded, in which case the blank is
    /// treated as clear and split down the middle.
    ///
    /// `padding` is the wrong source. A blank sized 140x150 by hand around
    /// a 100x100 model has a 20 mm clear strip while `padding` still
    /// reads 5.
    pub model_x_range: Option<(f64, f64)>,
    /// The face the flipped setup puts up.
    pub face_up: FaceUp,
    /// The turn of the flipped setup. With `face_up`, it sets the flip
    /// axis the pins must register.
    pub z_rotation: ZRotation,
    /// Pin diameter — the dowel, and therefore the hole, and therefore
    /// the drill. Sized from the tool the pin-drill operation will use,
    /// never a constant.
    pub pin_diameter: f64,
    /// Clearance from the pin's edge to the stock edge, to the model and
    /// to the other pins.
    pub wall_mm: f64,
}

/// Why no keyed pin pattern exists for this blank.
///
/// Every variant is a refusal, not a warning: a placement that hangs off
/// the stock or that seats turned is worse than no placement, because the
/// operator finds out at the flip.
#[derive(Debug, Clone, PartialEq)]
pub enum PinPlacementError {
    /// The placer works only for a `Bottom` setup at 0 deg, whose flip axis
    /// is the line `y = D/2` across the X clear strips. Other setups have
    /// no axis there, or no in-plane flip at all.
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
    /// The clear strip on one side cannot hold the pin plus its walls.
    /// This is the live wanaka case: a 5 mm ring, a 6 mm dowel.
    StripTooNarrow {
        side: PinSide,
        strip_mm: f64,
        required_mm: f64,
        pin_diameter_mm: f64,
    },
    /// The stock is not deep enough in Y for a mirror pair: two pins, a
    /// wall between them, and a wall to each edge.
    PairDoesNotFit {
        stock_d: f64,
        required_mm: f64,
        pin_diameter_mm: f64,
    },
    /// Every pair offset that fits gives a pattern that seats turned.
    CannotKey { best_miss_mm: f64, required_mm: f64 },
}

/// Which clear strip a placement failure refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinSide {
    /// The strip between `x = 0` and the model's minimum X.
    MinusX,
    /// The strip between the model's maximum X and `x = stock_w`.
    PlusX,
}

impl PinSide {
    pub fn label(&self) -> &'static str {
        match self {
            PinSide::MinusX => "-X",
            PinSide::PlusX => "+X",
        }
    }
}

impl std::fmt::Display for PinPlacementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PinPlacementError::UnsupportedFlip {
                face_up: FaceUp::Bottom,
                z_rotation,
            } => write!(
                f,
                "Cannot place registration pins automatically for a Bottom setup turned {}: \
                 auto-place works only at 0 deg, where the flip axis crosses the clear strips \
                 beside the part. Place the pins by hand: one pin on the flip axis and a mirror \
                 pair across it.",
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
            PinPlacementError::StripTooNarrow {
                side,
                strip_mm,
                required_mm,
                pin_diameter_mm,
            } => write!(
                f,
                "Cannot place registration pins: the clear strip on {} is {strip_mm:.1} mm, but a \
                 {pin_diameter_mm:.1} mm pin needs {required_mm:.1} mm (the pin plus a wall each \
                 side). Widen the stock, or fit a smaller dowel and drill.",
                side.label()
            ),
            PinPlacementError::PairDoesNotFit {
                stock_d,
                required_mm,
                pin_diameter_mm,
            } => write!(
                f,
                "Cannot place registration pins: the stock is {stock_d:.1} mm deep in Y, but a \
                 mirror pair of {pin_diameter_mm:.1} mm pins needs {required_mm:.1} mm (two pins, \
                 a wall between them and a wall to each edge). Deepen the stock, or fit a smaller \
                 dowel and drill."
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

/// Place a keyed pattern: one pin on the flip axis and a mirror pair off
/// the axis.
///
/// Two pins of one diameter never key: a half turn about their midpoint
/// puts each hole on the other dowel. One pin on the axis plus a mirror
/// pair is the smallest pattern of equal pins that seats under the flip
/// and keys, unless the three pins are in a line or form an equilateral
/// triangle.
///
/// The construction, in stock-local mm:
///
/// 1. Refuse anything but [`FaceUp::Bottom`] at [`ZRotation::Deg0`]. That
///    setup mirrors `y -> D - y`, so its axis is `y = D/2`, and the axis
///    crosses the X clear strips. (P6 adds the 2D placer for the others.)
/// 2. Take the two clear strips beside the model bbox, `0..model_min_x`
///    and `model_max_x..stock_w`. Not `padding`: padding describes how
///    the stock would be auto-sized, not how big it actually is.
/// 3. Put the axis pin at the centre of one strip, on `y = D/2`. Put the
///    pair at the centre of the other strip, at `y = D/2 +- h`. Each pin
///    maps to a pin under the flip, so the flip seats.
/// 4. Take the largest offset `h` that keeps a wall to each Y edge, then
///    step it down until the intrinsic check
///    ([`closest_wrong_seating`]) accepts the pattern. The first choice
///    is a pattern whose every wrong seating misses by a full pin
///    diameter (a dowel then does not overlap any hole). If no offset
///    gives that, the second choice is a pattern keyed at the bar
///    ([`keying_bar_mm`](super::keying_bar_mm)). The step is [`PIN_MATCH_TOL_MM`]: a finer step
///    moves a hole by less than the hole slop. A big `h` near the
///    equilateral spread seats turned 120 deg, so the first `h` is not
///    always accepted.
/// 5. Try the axis pin in each strip and keep the larger `h` (the wider
///    pattern holds the part more firmly). On a tie the axis pin goes in
///    the +X strip.
///
/// The result is `[axis pin, pair pin at low Y, pair pin at high Y]`.
pub fn place_keyed_pins(req: PinPlacementRequest) -> Result<[AlignmentPin; 3], PinPlacementError> {
    if req.face_up != FaceUp::Bottom || req.z_rotation != ZRotation::Deg0 {
        return Err(PinPlacementError::UnsupportedFlip {
            face_up: req.face_up,
            z_rotation: req.z_rotation,
        });
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

    // Clear strips either side of the part. With no model the blank is
    // entirely clear, so split it at the centre — that keeps both strips
    // well defined without inventing a keep-out.
    let (keep_lo, keep_hi) = match req.model_x_range {
        Some((lo, hi)) if hi > lo && lo.is_finite() && hi.is_finite() => {
            (lo.clamp(0.0, req.stock_w), hi.clamp(0.0, req.stock_w))
        }
        _ => (req.stock_w * 0.5, req.stock_w * 0.5),
    };
    let strip_lo = keep_lo;
    let strip_hi = req.stock_w - keep_hi;

    // Room a pin needs in a strip: its own diameter plus a wall to the
    // stock edge and a wall to the model.
    let required = req.pin_diameter + 2.0 * req.wall_mm;
    if strip_lo < required - PIN_GEOM_EPS_MM {
        return Err(PinPlacementError::StripTooNarrow {
            side: PinSide::MinusX,
            strip_mm: strip_lo,
            required_mm: required,
            pin_diameter_mm: req.pin_diameter,
        });
    }
    if strip_hi < required - PIN_GEOM_EPS_MM {
        return Err(PinPlacementError::StripTooNarrow {
            side: PinSide::PlusX,
            strip_mm: strip_hi,
            required_mm: required,
            pin_diameter_mm: req.pin_diameter,
        });
    }

    // The pair offset `h` from the axis. The smallest leaves a wall between
    // the two pair pins: 2h - d >= wall. The largest leaves a wall to each
    // Y edge: D/2 + h + d/2 + wall <= D.
    let r = 0.5 * req.pin_diameter;
    let h_min = 0.5 * (req.pin_diameter + req.wall_mm);
    let h_max = 0.5 * req.stock_d - r - req.wall_mm;
    if h_max < h_min - PIN_GEOM_EPS_MM {
        return Err(PinPlacementError::PairDoesNotFit {
            stock_d: req.stock_d,
            required_mm: 2.0 * req.pin_diameter + 3.0 * req.wall_mm,
            pin_diameter_mm: req.pin_diameter,
        });
    }

    let axis_y = 0.5 * req.stock_d;
    let centre_lo = 0.5 * strip_lo;
    let centre_hi = req.stock_w - 0.5 * strip_hi;
    let pattern = |axis_x: f64, pair_x: f64, h: f64| {
        [
            AlignmentPin::new(axis_x, axis_y, req.pin_diameter),
            AlignmentPin::new(pair_x, axis_y - h, req.pin_diameter),
            AlignmentPin::new(pair_x, axis_y + h, req.pin_diameter),
        ]
    };

    // The largest pair offset, from `h_max` down in steps, whose pattern
    // `accept` takes. The axis pin goes in the +X strip first, so a tie
    // keeps it there.
    let mut best_miss: f64 = 0.0;
    let mut largest = |accept: &dyn Fn(&[AlignmentPin; 3]) -> bool| {
        let mut best: Option<(f64, [AlignmentPin; 3])> = None;
        for (axis_x, pair_x) in [(centre_hi, centre_lo), (centre_lo, centre_hi)] {
            let mut h = h_max.max(h_min);
            loop {
                let pins = pattern(axis_x, pair_x, h);
                if accept(&pins) {
                    if best
                        .as_ref()
                        .is_none_or(|(best_h, _)| h > *best_h + PIN_GEOM_EPS_MM)
                    {
                        best = Some((h, pins));
                    }
                    break;
                }
                if let Some(w) = closest_wrong_seating(&pins) {
                    best_miss = best_miss.max(w.miss_mm);
                }
                if h <= h_min + PIN_GEOM_EPS_MM {
                    break;
                }
                h = (h - PIN_MATCH_TOL_MM).max(h_min);
            }
        }
        best
    };
    // First choice: every wrong seating misses by a full pin diameter, so
    // a dowel does not overlap any hole at all. Second choice: keyed at
    // the bar (the dowel centre lands outside the hole edge).
    let clear = |pins: &[AlignmentPin; 3]| {
        closest_wrong_seating(pins).is_none_or(|w| w.miss_mm >= req.pin_diameter - PIN_GEOM_EPS_MM)
    };
    let keyed = |pins: &[AlignmentPin; 3]| pattern_is_keyed(pins);
    let best = largest(&clear).or_else(|| largest(&keyed));

    best.map(|(_, pins)| pins)
        .ok_or(PinPlacementError::CannotKey {
            best_miss_mm: best_miss,
            // The bar of a pattern of equal pins is their radius.
            required_mm: 0.5 * req.pin_diameter,
        })
}
