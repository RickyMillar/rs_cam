use serde::{Deserialize, Serialize};

use crate::compute::transform::FaceUp;
use crate::geo::BoundingBox3;

/// Unique identifier for a loaded model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelId(pub usize);

/// Unique identifier for a setup (workholding / orientation context).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SetupId(pub usize);

/// Unique identifier for a fixture within a setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FixtureId(pub usize);

/// Unique identifier for a keep-out zone within a setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeepOutId(pub usize);

/// What kind of geometry was loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Stl,
    Svg,
    Dxf,
    Step,
}

/// Assumed units of the imported STL (determines scale factor to mm).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "scale", rename_all = "snake_case")]
pub enum ModelUnits {
    Millimeters,
    Inches,
    Meters,
    Centimeters,
    Custom(f64),
}

impl ModelUnits {
    pub const PRESETS: &[(ModelUnits, &'static str)] = &[
        (ModelUnits::Millimeters, "mm (1:1)"),
        (ModelUnits::Inches, "inches (x25.4)"),
        (ModelUnits::Centimeters, "cm (x10)"),
        (ModelUnits::Meters, "m (x1000)"),
    ];

    pub fn scale_factor(&self) -> f64 {
        match self {
            ModelUnits::Millimeters => 1.0,
            ModelUnits::Inches => 25.4,
            ModelUnits::Meters => 1000.0,
            ModelUnits::Centimeters => 10.0,
            ModelUnits::Custom(s) => *s,
        }
    }

    pub fn label(&self) -> String {
        match self {
            ModelUnits::Millimeters => "mm".into(),
            ModelUnits::Inches => "inches".into(),
            ModelUnits::Meters => "m".into(),
            ModelUnits::Centimeters => "cm".into(),
            ModelUnits::Custom(s) => format!("x{s:.3}"),
        }
    }
}

// Re-export PostFormat from core (single source of truth).
pub use crate::gcode::PostFormat;

/// Post-processor configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostConfig {
    pub format: PostFormat,
    pub spindle_speed: u32,
    pub safe_z: f64,
    /// Convert G0 rapids to G1 at high feedrate (for machines with unpredictable rapid behavior).
    pub high_feedrate_mode: bool,
    pub high_feedrate: f64,
    /// Project-level spindle policy used by Suggest. `MatchChart`
    /// (default) preserves chart RPM. `MaxSpeed` walks the constant-
    /// chipload line up to the spindle ceiling, scaling feed
    /// proportionally. See [`crate::feeds::SpindleStrategy`].
    #[serde(default)]
    pub spindle_strategy: crate::feeds::SpindleStrategy,
}

impl Default for PostConfig {
    fn default() -> Self {
        Self {
            format: PostFormat::Grbl,
            spindle_speed: 18000,
            safe_z: 10.0,
            high_feedrate_mode: false,
            high_feedrate: 5000.0,
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        }
    }
}

/// Which axis the stock flips about when changing setups.
///
/// **Derived, not authored.** The authoritative statement of how a blank
/// is re-seated is the setup's [`FaceUp`](crate::compute::transform::FaceUp);
/// this enum is a cache of that, kept because the viewport draws the flip
/// centre line from it and saved projects carry the key. Build it with
/// [`FlipAxis::from_face_up`] rather than asking a human — a value that
/// disagrees with the setups is how G-PINAUTO shipped a `null` flip axis
/// alongside a `FaceUp::Bottom` setup.
///
/// The pre-2026-08-22 doc comment on these variants contradicted itself
/// ("mirror about the X centerline" reflects Y; "Y stays" reflects X).
/// The behaviour was always the first reading, and that is what
/// `FaceUp::Bottom` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlipAxis {
    /// Mirror about the X centre line: `y -> stock_y - y`, X preserved.
    /// This is the one flip the CAM models — [`FaceUp::Bottom`].
    Horizontal,
    /// Mirror about the Y centre line: `x -> stock_x - x`, Y preserved.
    ///
    /// **No `FaceUp` produces this map.** Retained only so projects saved
    /// before 2026-08-22 still load and render; nothing authors it.
    Vertical,
}

impl FlipAxis {
    pub fn label(&self) -> &'static str {
        match self {
            FlipAxis::Horizontal => "Horizontal",
            FlipAxis::Vertical => "Vertical",
        }
    }

    /// The in-plane mirror a setup's `face_up` performs, if it performs one.
    ///
    /// Only `Bottom` keeps the part's XY footprint and mirrors within it.
    /// `Front/Back/Left/Right` stand the blank on an edge — the footprint
    /// becomes a different pair of stock dimensions entirely
    /// (`FaceUp::transform_dims`), so no in-plane pin pattern registers
    /// them and there is nothing honest to return.
    pub fn from_face_up(face_up: FaceUp) -> Option<Self> {
        match face_up {
            FaceUp::Bottom => Some(FlipAxis::Horizontal),
            FaceUp::Top | FaceUp::Front | FaceUp::Back | FaceUp::Left | FaceUp::Right => None,
        }
    }
}

/// A physical alignment pin position for part registration between setups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentPin {
    pub x: f64,
    pub y: f64,
    pub diameter: f64,
}

impl AlignmentPin {
    pub fn new(x: f64, y: f64, diameter: f64) -> Self {
        Self { x, y, diameter }
    }
}

// ── Alignment-pin keying (G-PINAUTO, 2026-08-22) ─────────────────────────
//
// A rectangular blank drops back onto its dowels four ways: identity,
// `My` (mirror about the X centre line, `y -> D - y`), `Mx`
// (`x -> W - x`), and `R180 = Mx . My`. The CAM models exactly one of
// them — `FaceUp::Bottom` is `(x, D - y, H - z)` — so a pin pattern is
// only correct when it is invariant under `My` and invariant under
// NOTHING ELSE. Invariant under `My` or the flip does not seat;
// invariant under `Mx` or `R180` as well and the operator can seat the
// part wrong without noticing.
//
// Both conditions are met by the same two-pin pattern: put both pins ON
// the mirror line `y = D/2` (every pin then maps to itself under `My`),
// and make the x-multiset fail `x -> W - x` invariance by more than hole
// slop. See planning/airrun_2026-08-19/RUN_LOG.md "## G-PINAUTO".

/// Clearance demanded between a pin's physical edge and any boundary —
/// the stock edge on one side, the model bbox on the other.
pub const PIN_WALL_MM: f64 = 2.0;

/// How far the pin pair must miss centre-symmetry before the wrong
/// seating counts as mechanically blocked.
///
/// A slip-fit dowel has on the order of 0.1 mm of play, so a few tenths
/// of asymmetry can be forced by a determined operator with a mallet.
/// 3 mm cannot: the hole simply is not there.
pub const PIN_KEYING_ASYMMETRY_MM: f64 = 3.0;

/// Two pin positions closer than this are the same physical hole — a
/// dowel will drop into either. Also the bar for "this re-seating is
/// blocked": a miss smaller than this still seats.
pub const PIN_MATCH_TOL_MM: f64 = 0.5;

/// Slop on the comparisons below, so a placement that achieves exactly
/// the required asymmetry is not rejected by its own rounding.
const PIN_GEOM_EPS_MM: f64 = 1e-9;

/// Everything [`plan_keyed_pins`](StockConfig::plan_keyed_pins) needs,
/// in **stock-local** millimetres (X0Y0 at the stock's min corner —
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
    /// This is the whole point of the rewrite: `padding` is the wrong
    /// source. A blank sized 140x150 by hand around a 100x100 model has a
    /// 20 mm clear strip while `padding` still reads 5.
    pub model_x_range: Option<(f64, f64)>,
    /// The flip the pins have to register.
    pub face_up: FaceUp,
    /// Pin diameter — the dowel, and therefore the hole, and therefore
    /// the drill. Sized from the tool the pin-drill operation will use,
    /// never a constant.
    pub pin_diameter: f64,
    /// Clearance from the pin's edge to the stock edge and to the model.
    pub wall_mm: f64,
}

/// Why no keyed pin pair exists for this blank.
///
/// Every variant is a refusal, not a warning: emitting a placement that
/// hangs off the stock or that seats four ways is worse than emitting
/// none, because the operator finds out at the flip.
#[derive(Debug, Clone, PartialEq)]
pub enum PinPlacementError {
    /// `Front/Back/Left/Right` stand the blank on an edge and change its
    /// footprint; `Top` is not a flip at all. Only `Bottom` is in-plane.
    UnsupportedFlip { face_up: FaceUp },
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
    /// Both pins fit, but not with enough freedom left to break
    /// centre-symmetry — so the pair would seat all four ways.
    CannotKey { available_mm: f64, required_mm: f64 },
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
            PinPlacementError::UnsupportedFlip { face_up } => write!(
                f,
                "Cannot place registration pins for a '{}' setup: only a Bottom flip keeps the \
                 part's XY footprint, the edge-up orientations change which stock dimensions the \
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
            PinPlacementError::CannotKey {
                available_mm,
                required_mm,
            } => write!(
                f,
                "Cannot place registration pins: the pins fit, but only {available_mm:.1} mm of \
                 play is left to offset them, and {required_mm:.1} mm is needed to stop the part \
                 seating the wrong way round. Widen the stock, or fit a smaller dowel."
            ),
        }
    }
}

/// What a stored pin set actually does under a given flip.
///
/// Reports **millimetres of mismatch**, not booleans, so a caller can
/// distinguish "off by 0.2 mm, a dowel will still find it" from "off by
/// 145 mm, there is no hole there".
#[derive(Debug, Clone, PartialEq)]
pub struct PinFlipReport {
    /// The flip the pins were judged against.
    pub face_up: FaceUp,
    /// `false` for the edge-up orientations and for `Top` — the numbers
    /// below are then meaningless and [`Self::warnings`] says so.
    pub modelled_flip: bool,
    pub pin_count: usize,
    /// Worst distance, over all pins, from a pin's image under the
    /// modelled flip to the nearest actual pin. `0` means every pin maps
    /// onto a hole, i.e. the part re-seats.
    pub seat_mismatch_mm: f64,
    /// Same measure under `Mx` (`x -> W - x`). LARGE is good: it is how
    /// far the part is from seating mirrored.
    pub mirror_x_mismatch_mm: f64,
    /// Same measure under `R180`. LARGE is good.
    pub rot180_mismatch_mm: f64,
    /// Pin indices whose body is not wholly inside the stock.
    pub out_of_bounds: Vec<usize>,
}

impl PinFlipReport {
    /// Does the part re-seat after the flip it is programmed for?
    pub fn seats(&self) -> bool {
        self.modelled_flip && self.pin_count > 0 && self.seat_mismatch_mm <= PIN_MATCH_TOL_MM
    }

    /// How badly the two wrong seatings are blocked, in mm. The weaker
    /// of the two is what an operator would find first.
    pub fn keying_margin_mm(&self) -> f64 {
        self.mirror_x_mismatch_mm.min(self.rot180_mismatch_mm)
    }

    /// Is the wrong orientation blocked by more than hole slop can force?
    pub fn keyed(&self) -> bool {
        self.pin_count > 0 && self.keying_margin_mm() >= PIN_KEYING_ASYMMETRY_MM - PIN_GEOM_EPS_MM
    }

    /// Loader-ready warning lines. Empty when there is nothing to say —
    /// including when there are no pins at all, which is a different
    /// question than whether the pins that exist are correct.
    pub fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.pin_count == 0 {
            return out;
        }
        // Bounds are a property of the pins alone, so they are reported
        // whatever the setup does; seating and keying only mean anything
        // against a flip the CAM actually models.
        if !self.out_of_bounds.is_empty() {
            out.push(format!(
                "{} alignment pin(s) are not wholly inside the stock.",
                self.out_of_bounds.len()
            ));
        }
        if !self.modelled_flip {
            return out;
        }
        if !self.seats() {
            out.push(format!(
                "Alignment pins do not survive the flip: after the '{}' flip the worst pin misses \
                 its hole by {:.1} mm. The part cannot be re-seated on these dowels.",
                self.face_up.label(),
                self.seat_mismatch_mm
            ));
        }
        if !self.keyed() {
            out.push(format!(
                "Alignment pins do not key the flip: the pattern is only {:.1} mm away from \
                 seating in the wrong orientation (need {PIN_KEYING_ASYMMETRY_MM:.1} mm). The \
                 part can be dropped on 180 deg out.",
                self.keying_margin_mm()
            ));
        }
        out
    }
}

/// Worst-case, over all pins, of the distance from `map(pin)` to the
/// nearest pin in the set.
///
/// For the blank to seat under `map`, EVERY pin has to find a hole, so
/// the worst offender is what decides it — which makes the same number
/// serve both questions: near zero means "seats", large means "blocked
/// by this much".
fn reseat_mismatch_mm(pins: &[AlignmentPin], map: impl Fn(&AlignmentPin) -> (f64, f64)) -> f64 {
    let mut worst: f64 = 0.0;
    for pin in pins {
        let (mx, my) = map(pin);
        let mut nearest = f64::INFINITY;
        for other in pins {
            let d = ((other.x - mx).powi(2) + (other.y - my).powi(2)).sqrt();
            if d < nearest {
                nearest = d;
            }
        }
        if nearest.is_finite() && nearest > worst {
            worst = nearest;
        }
    }
    worst
}

/// Stock material configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockConfig {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub origin_x: f64,
    pub origin_y: f64,
    pub origin_z: f64,
    pub auto_from_model: bool,
    pub padding: f64,
    pub material: crate::material::Material,
    /// Alignment pins for multi-setup registration (stock-level, persists
    /// across flips). Stock-LOCAL: X0Y0 at the stock's min corner.
    #[serde(default)]
    pub alignment_pins: Vec<AlignmentPin>,
    /// Cached [`FlipAxis`] for the viewport's flip centre line and the
    /// stock panel's Mirror affordance.
    ///
    /// **Derived from the setups, not authored, and not a constraint.**
    /// Build it with [`FlipAxis::from_face_up`]. Pin correctness is
    /// judged by [`Self::validate_pins_for_flip`] against the setup's
    /// `face_up`, never against this field — a stored axis that
    /// disagrees with the setups is a lie, and a `null` one beside a
    /// `Bottom` setup is what G-PINAUTO shipped.
    ///
    /// Invariant the GUI maintains: `Some(..)` only while
    /// `alignment_pins` is non-empty. A flipped setup whose pin
    /// placement was refused leaves this `None`, which is what keeps
    /// that state reading as INCOMPLETE rather than configured. Being a
    /// cache, it may still be stale in a file saved elsewhere; readers
    /// must treat it as advisory and never as evidence about the pins.
    #[serde(default)]
    pub flip_axis: Option<FlipAxis>,
    /// Workholding rigidity for feeds calculation.
    #[serde(default = "default_workholding_rigidity")]
    pub workholding_rigidity: crate::feeds::WorkholdingRigidity,
}

fn default_workholding_rigidity() -> crate::feeds::WorkholdingRigidity {
    crate::feeds::WorkholdingRigidity::Medium
}

impl Default for StockConfig {
    fn default() -> Self {
        Self {
            x: 100.0,
            y: 100.0,
            z: 25.0,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            auto_from_model: true,
            padding: 5.0,
            material: crate::material::Material::default(),
            alignment_pins: Vec::new(),
            flip_axis: None,
            workholding_rigidity: crate::feeds::WorkholdingRigidity::Medium,
        }
    }
}

impl StockConfig {
    /// Update stock dimensions from a model bounding box.
    ///
    /// XY dimensions always follow the bbox extent + padding.
    ///
    /// Z handling depends on whether the bbox is 3D or 2D:
    /// - **3D** (mesh with non-zero Z extent): stock Z follows the bbox
    ///   Z range + padding, and `origin_z` is set to `bbox.min.z`.
    /// - **2D** (SVG/DXF polygon with `min.z == max.z`): the stock's
    ///   existing Z dimension is preserved (the user's default
    ///   thickness), but `origin_z` is set to `bbox.min.z − self.z`
    ///   so the stock's **top** is at `bbox.min.z`. Pocket / adaptive /
    ///   profile / trace / v-carve / chamfer / drill operations on 2D
    ///   models cut at negative Z (cut_depth < 0 relative to the
    ///   model's Z plane), so the stock must span `[bbox.min.z − z,
    ///   bbox.min.z]` for the cuts to engage material. Without this
    ///   shift, every cutting move was below the stock floor and the
    ///   simulator reported 0 engagement (F-2 in the April review).
    ///
    /// See planning/adaptive_review_2026-04.md F-13 (XY auto-size) and
    /// F-2 (2D Z frame).
    pub fn update_from_bbox(&mut self, bbox: &BoundingBox3) {
        self.x = bbox.max.x - bbox.min.x + 2.0 * self.padding;
        self.y = bbox.max.y - bbox.min.y + 2.0 * self.padding;
        self.origin_x = bbox.min.x - self.padding;
        self.origin_y = bbox.min.y - self.padding;
        let bbox_z_range = bbox.max.z - bbox.min.z;
        if bbox_z_range > 0.0 {
            self.z = bbox_z_range + self.padding;
            self.origin_z = bbox.min.z;
        } else {
            // 2D model: place stock so its TOP is at bbox.min.z (= 0 for
            // SVG/DXF). Leaves self.z unchanged; only origin_z shifts.
            self.origin_z = bbox.min.z - self.z;
        }
    }

    /// Get the bounding box of the stock.
    pub fn bbox(&self) -> BoundingBox3 {
        use crate::geo::P3;
        BoundingBox3 {
            min: P3::new(self.origin_x, self.origin_y, self.origin_z),
            max: P3::new(
                self.origin_x + self.x,
                self.origin_y + self.y,
                self.origin_z + self.z,
            ),
        }
    }

    /// Two registration pins that key `face_up`, or a stated reason why
    /// none exist.
    ///
    /// `model_bbox` is in WORLD mm (as models carry it); the stock origin
    /// is subtracted here because `alignment_pins` are stock-local. Pass
    /// `None` only when no model is loaded — passing `None` to dodge a
    /// refusal puts pins through the part.
    ///
    /// `pin_diameter` must come from the tool the pin-drill operation
    /// will run. A hole planned at 6 mm and drilled with a 3 mm cutter is
    /// a hole no 6 mm dowel ever sees, and the wall check above was then
    /// computed against a pin that does not exist.
    pub fn plan_keyed_pins(
        &self,
        face_up: FaceUp,
        model_bbox: Option<&BoundingBox3>,
        pin_diameter: f64,
    ) -> Result<[AlignmentPin; 2], PinPlacementError> {
        let model_x_range = model_bbox.map(|b| (b.min.x - self.origin_x, b.max.x - self.origin_x));
        place_keyed_pins(PinPlacementRequest {
            stock_w: self.x,
            stock_d: self.y,
            model_x_range,
            face_up,
            pin_diameter,
            wall_mm: PIN_WALL_MM,
        })
    }

    /// What this stock's stored pins do under `face_up`.
    ///
    /// Reachable from anywhere in core, which is the point: the live
    /// defect was a saved project whose pins were centre-symmetric —
    /// invariant under a 180 deg ROTATION, which is not the flip — and no
    /// load path looked. Call this on load and publish
    /// [`PinFlipReport::warnings`].
    pub fn validate_pins_for_flip(&self, face_up: FaceUp) -> PinFlipReport {
        validate_pins_for_flip(&self.alignment_pins, face_up, self.x, self.y)
    }
}

/// Place two pins that seat under the modelled flip and under nothing else.
///
/// The construction, in stock-local mm:
///
/// 1. Refuse anything but [`FaceUp::Bottom`] — the other orientations do
///    not preserve the XY footprint the pins are dimensioned in.
/// 2. Take the two clear strips beside the model bbox, `0..model_min_x`
///    and `model_max_x..stock_w`. Not `padding`: padding describes how
///    the stock would be auto-sized, not how big it actually is.
/// 3. Both pins go on `y = stock_d / 2`, the flip's mirror line, where
///    `My` maps each pin onto itself. That is what makes the flip seat.
/// 4. Slide the pair along that line until the x-multiset is no longer
///    invariant under `x -> stock_w - x`, by at least
///    [`PIN_KEYING_ASYMMETRY_MM`]. That is what blocks `Mx` and `R180`.
///    The slide is split between the two pins in proportion to the play
///    each strip has, so neither pin eats its wall clearance first.
///
/// Note that an ODD pin count buys nothing here: a pin at `x = W/2` is
/// its own image under `x -> W - x`. Extra pins buy redundancy against
/// rocking, never keying — only the x-multiset asymmetry keys.
pub fn place_keyed_pins(req: PinPlacementRequest) -> Result<[AlignmentPin; 2], PinPlacementError> {
    if req.face_up != FaceUp::Bottom {
        return Err(PinPlacementError::UnsupportedFlip {
            face_up: req.face_up,
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
    // stock edge and a wall to the model. This is the check that was
    // missing — margin = padding/2 put a 6 mm hole 2.5 mm from the edge,
    // hanging 0.5 mm off the blank.
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

    // Play left in each strip once the pin and its walls are seated. A
    // pin centred in its strip can travel half of this either way.
    // Clamped at zero: the checks above admit a strip that is short by
    // the epsilon, and a negative slack would invert the split below.
    let slack_lo = (strip_lo - required).max(0.0);
    let slack_hi = (strip_hi - required).max(0.0);
    let centre_lo = strip_lo * 0.5;
    let centre_hi = req.stock_w - strip_hi * 0.5;

    // Asymmetry the centred pair already has, signed. Unequal strips key
    // the part for free; equal strips give exactly zero and must be slid.
    let asym_centred = centre_lo + centre_hi - req.stock_w;
    let shift_total = if asym_centred.abs() >= PIN_KEYING_ASYMMETRY_MM {
        0.0
    } else if asym_centred >= 0.0 {
        PIN_KEYING_ASYMMETRY_MM - asym_centred
    } else {
        -PIN_KEYING_ASYMMETRY_MM - asym_centred
    };

    // Sliding both pins the same way changes the x-multiset's symmetry
    // without moving either off the mirror line, so the flip still seats.
    // Total slide available is half the combined play.
    let capacity = 0.5 * (slack_lo + slack_hi);
    if shift_total.abs() > capacity + PIN_GEOM_EPS_MM {
        return Err(PinPlacementError::CannotKey {
            available_mm: capacity,
            required_mm: PIN_KEYING_ASYMMETRY_MM,
        });
    }

    let total_slack = slack_lo + slack_hi;
    let (shift_lo, shift_hi) = if total_slack > 0.0 {
        (
            shift_total * slack_lo / total_slack,
            shift_total * slack_hi / total_slack,
        )
    } else {
        // Zero play both sides; only reachable when no slide was needed.
        (0.0, 0.0)
    };

    let y = req.stock_d * 0.5;
    Ok([
        AlignmentPin::new(centre_lo + shift_lo, y, req.pin_diameter),
        AlignmentPin::new(centre_hi + shift_hi, y, req.pin_diameter),
    ])
}

/// Judge an existing pin set against the flip it is supposed to register.
///
/// See [`PinFlipReport`]. `stock_w` / `stock_d` are the stock XY extents;
/// pins are stock-local.
pub fn validate_pins_for_flip(
    pins: &[AlignmentPin],
    face_up: FaceUp,
    stock_w: f64,
    stock_d: f64,
) -> PinFlipReport {
    let modelled_flip = FlipAxis::from_face_up(face_up).is_some();
    let out_of_bounds = pins
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            let r = p.diameter * 0.5;
            p.x - r < 0.0 || p.x + r > stock_w || p.y - r < 0.0 || p.y + r > stock_d
        })
        .map(|(i, _)| i)
        .collect();

    PinFlipReport {
        face_up,
        modelled_flip,
        pin_count: pins.len(),
        // My: the modelled flip. X preserved, Y mirrored about D/2.
        seat_mismatch_mm: reseat_mismatch_mm(pins, |p| (p.x, stock_d - p.y)),
        // Mx: mirrored the other way. Must MISS.
        mirror_x_mismatch_mm: reseat_mismatch_mm(pins, |p| (stock_w - p.x, p.y)),
        // R180 = Mx . My. Must MISS. This is the one wanaka's diagonal
        // pins were invariant under, which is why they looked plausible.
        rot180_mismatch_mm: reseat_mismatch_mm(pins, |p| (stock_w - p.x, stock_d - p.y)),
        out_of_bounds,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::geo::P3;

    #[test]
    fn update_from_bbox_updates_xy_from_2d_bbox() {
        let mut stock = StockConfig {
            x: 100.0,
            y: 100.0,
            z: 25.0,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            padding: 5.0,
            ..StockConfig::default()
        };
        // 2D SVG/DXF polygon bbox: zero Z range
        let bbox = BoundingBox3 {
            min: P3::new(5.0, 5.0, 0.0),
            max: P3::new(35.0, 35.0, 0.0),
        };
        stock.update_from_bbox(&bbox);
        // XY should have been updated to fit 30x30 polygon + 5mm padding each side
        assert!((stock.x - 40.0).abs() < 1e-9);
        assert!((stock.y - 40.0).abs() < 1e-9);
        assert!((stock.origin_x - 0.0).abs() < 1e-9);
        assert!((stock.origin_y - 0.0).abs() < 1e-9);
        // Z dimension should be UNCHANGED from 25.0 — F-13 requires the
        // user's default stock thickness survives attaching a 2D model.
        assert!((stock.z - 25.0).abs() < 1e-9);
        // Z origin shifts so the stock TOP is at bbox.min.z (= 0 for
        // the SVG frame). Before F-2, origin_z stayed at 0 which put
        // the stock entirely below the 2D pocket's cut range.
        assert!(
            (stock.origin_z - (-25.0)).abs() < 1e-9,
            "expected 2D stock to have origin_z = -z (top at 0), got {}",
            stock.origin_z
        );
    }

    #[test]
    fn update_from_bbox_updates_z_from_3d_bbox() {
        let mut stock = StockConfig {
            x: 100.0,
            y: 100.0,
            z: 25.0,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            padding: 5.0,
            ..StockConfig::default()
        };
        // 3D mesh bbox with non-zero Z extent
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(50.0, 40.0, 10.0),
        };
        stock.update_from_bbox(&bbox);
        assert!((stock.x - 60.0).abs() < 1e-9); // 50 + 2*5
        assert!((stock.y - 50.0).abs() < 1e-9); // 40 + 2*5
        assert!((stock.z - 15.0).abs() < 1e-9); // 10 + 5
        assert!((stock.origin_z - 0.0).abs() < 1e-9);
    }
}
