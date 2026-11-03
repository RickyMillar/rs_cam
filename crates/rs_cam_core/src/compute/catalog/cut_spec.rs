//! CONTRACT K1: every op states its cut (feeds contract P4).
//!
//! [`CutSpec`] is the cut of one op: the radial cut, the axial cut, the
//! total depth and what the first pass meets. `OperationConfig::cut_spec`
//! builds it from the op's own config. That match names every variant and
//! has no wildcard arm.
//!
//! [`CutRule`] is the row half. Each `REG_*` row states the rule that its
//! `cut_spec` arm follows. The row holds a rule, never a number.
//!
//! No type in this file has `Default`. A [`Stated`] value has a constructor
//! that only `compute/` can call, so Suggest (`feeds/`) cannot make an
//! operator value. The refusals are `compile_fail` doctests on
//! [`cut_spec_refusals`].

/// A length in millimetres. It keeps the `f64` bit for bit: no rounding,
/// no unit conversion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mm(f64);

impl Mm {
    /// The length `mm` (millimetres), bit for bit.
    #[must_use]
    pub const fn new(mm: f64) -> Self {
        Self(mm)
    }

    /// The length in millimetres.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// A value that the operator set in the op config. Only `compute/` can
/// make one, so Suggest cannot write a value and mark it "operator".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stated<T>(T);

impl<T: Copy> Stated<T> {
    /// Marks `value` as the operator's value. Only the op's own config
    /// calls this (`OperationConfig::cut_spec`).
    pub(in crate::compute) const fn new(value: T) -> Self {
        Self(value)
    }

    /// The operator's value.
    #[must_use]
    pub const fn get(self) -> T {
        self.0
    }
}

impl Stated<Mm> {
    /// The operator's value in millimetres, bit for bit.
    #[must_use]
    pub const fn mm(self) -> f64 {
        self.0.get()
    }
}

/// CONTRACT K1. Built only by `OperationConfig::cut_spec` (exhaustive, no
/// wildcard). No `Default` on any type in this file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutSpec {
    /// The radial cut.
    pub radial: Radial,
    /// The axial cut.
    pub axial: Axial,
    /// The total depth of the op.
    pub total: TotalDepth,
    /// What the first pass meets.
    pub first_pass: FirstPass,
}

/// The radial cut of an op.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Radial {
    /// The stepover that the operator set.
    Width(Stated<Mm>),
    /// The full tool diameter (ae = D).
    Slot,
    /// A V groove: the width follows the depth. The tool angle is not here,
    /// because `cut_spec` reads the config only (P4 decision D2).
    Groove,
    /// A finish: the width comes from the scallop target.
    Scallop(Stated<Mm>),
    /// Radial Finish: the angle in degrees between two spokes (D2).
    AngularStep(Stated<f64>),
    /// The surface sets the width.
    SurfaceFollow,
    /// Drill, Pin Drill: no radial cut.
    Axial,
}

impl Radial {
    /// The scallop target (mm), if this cut has one.
    #[must_use]
    pub const fn scallop_mm(&self) -> Option<f64> {
        match self {
            Self::Scallop(s) => Some(s.mm()),
            Self::Width(_)
            | Self::Slot
            | Self::Groove
            | Self::AngularStep(_)
            | Self::SurfaceFollow
            | Self::Axial => None,
        }
    }
}

/// The axial cut of an op.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Axial {
    /// The depth per pass that the operator set.
    PerPass(Stated<Mm>),
    /// The Z step between two levels.
    ZStep(Stated<Mm>),
    /// One pass to this depth. `None`: the depth is not set (S11).
    OnePass(Option<Stated<Mm>>),
    /// The peck depth of a drill cycle.
    Peck(Stated<Mm>),
    /// Bore: the Z drop of one helix turn (D2).
    HelixPitch(Stated<Mm>),
    /// Chamfer: the chamfer width (D2).
    ChamferWidth(Stated<Mm>),
    /// The surface sets the depth. `max` is the operator's cap, if any.
    Surface {
        /// The largest axial depth that the operator set.
        max: Option<Stated<Mm>>,
    },
}

impl Axial {
    /// The axial value (mm) that the operator set, if this cut has one.
    #[must_use]
    pub const fn stated_mm(&self) -> Option<f64> {
        match self {
            Self::PerPass(s)
            | Self::ZStep(s)
            | Self::Peck(s)
            | Self::HelixPitch(s)
            | Self::ChamferWidth(s) => Some(s.mm()),
            Self::OnePass(s) | Self::Surface { max: s } => match s {
                Some(s) => Some(s.mm()),
                None => None,
            },
        }
    }
}

/// The total depth of an op.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TotalDepth {
    /// The config gives the total depth.
    Known(Stated<Mm>),
    /// The resolved heights give the total depth (the floor).
    FromHeights,
    /// The config field that gives the depth is not set (S11).
    Unset,
    /// No single total depth. The text gives the reason.
    None(&'static str),
}

/// What the first pass meets (P4 decision D1). The stock source is not in
/// the config, so P4 has no fresh-stock or rest-stock value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstPass {
    /// The first pass has material on both sides (ae = D).
    Slot,
    /// The first pass is not a slot.
    NotSlot,
}

/// CONTRACT K1 (row half). No `Default`, no `NONE` constant: each `REG_*`
/// literal names every field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutRule {
    /// The rule of [`CutSpec::radial`].
    pub radial: RadialRule,
    /// The rule of [`CutSpec::axial`].
    pub axial: AxialRule,
}

/// The rule that gives [`Radial`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialRule {
    /// [`Radial::Width`] from the `stepover` field.
    Stepover,
    /// [`Radial::Slot`].
    Slot,
    /// [`Radial::Groove`].
    Groove,
    /// [`Radial::Scallop`] from the `scallop_height` field.
    Scallop,
    /// [`Radial::Scallop`] when `scallop_height` is set, else
    /// [`Radial::Width`] from `stepover`.
    ScallopElseStepover,
    /// [`Radial::Width`] from `offset_stepover` when `num_offset_passes`
    /// is more than 1, else [`Radial::SurfaceFollow`].
    OffsetStepoverElseSurface,
    /// [`Radial::AngularStep`] from the `angular_step` field.
    AngularStep,
    /// [`Radial::SurfaceFollow`].
    SurfaceFollow,
    /// [`Radial::Axial`]: no radial cut.
    NoRadial,
}

/// The rule that gives [`Axial`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxialRule {
    /// [`Axial::PerPass`] from the `depth_per_pass` field.
    DepthPerPass,
    /// [`Axial::ZStep`] from the `z_step` field.
    ZStep,
    /// [`Axial::OnePass`] from the op's depth field.
    OnePass,
    /// [`Axial::Peck`] from the `peck_depth` field.
    Peck,
    /// [`Axial::HelixPitch`] from the `pitch` field.
    HelixPitch,
    /// [`Axial::ChamferWidth`] from the `chamfer_width` field.
    ChamferWidth,
    /// [`Axial::Surface`] with no cap.
    Surface,
    /// [`Axial::Surface`] with the cap from the op's cap field.
    SurfaceCapped,
}

impl CutSpec {
    /// CONTRACT K2 (feeds contract P5): the calculator inputs of the cut,
    /// in the order `(axial_depth, radial_width, scallop)`. P5 replaces the
    /// P4 marker `PresentBasis`: the calculator reads the operator's cut,
    /// not a family default, wherever the spec states it.
    ///
    /// - The depth: a depth per pass, a Z step, a one-pass depth, or the
    ///   operator's surface cap, when it is above zero. A peck, a helix
    ///   pitch and a chamfer width are not a side-cut depth, so the
    ///   calculator reads none for them.
    /// - The width: a stepover, or the tool diameter for a slot. A groove,
    ///   a surface width, an angular step and a drill give none.
    /// - The scallop target, when the radial cut is a scallop.
    #[must_use]
    pub const fn calc_inputs(&self, diameter_mm: f64) -> (Option<f64>, Option<f64>, Option<f64>) {
        let ap = match self.axial {
            Axial::PerPass(s) | Axial::ZStep(s) => Some(s.mm()),
            Axial::OnePass(s) | Axial::Surface { max: s } => match s {
                Some(s) => Some(s.mm()),
                None => None,
            },
            Axial::Peck(_) | Axial::HelixPitch(_) | Axial::ChamferWidth(_) => None,
        };
        // A depth at or below zero is an offset from a surface (Project
        // Curve), not a cut depth: the calculator reads none.
        let ap = match ap {
            Some(v) if v > 0.0 => Some(v),
            Some(_) | None => None,
        };
        let ae = match self.radial {
            Radial::Width(s) => Some(s.mm()),
            Radial::Slot => Some(diameter_mm),
            Radial::Groove
            | Radial::Scallop(_)
            | Radial::AngularStep(_)
            | Radial::SurfaceFollow
            | Radial::Axial => None,
        };
        // A width at or below zero is not a cut width: the calculator reads
        // none (the same rule as the depth above).
        let ae = match ae {
            Some(v) if v > 0.0 => Some(v),
            Some(_) | None => None,
        };
        (ap, ae, self.radial.scallop_mm())
    }
}

/// The K1 refusals of the compiler. Each `compile_fail` block has a
/// sibling block that compiles.
///
/// A row rule has no `Default`:
///
/// ```compile_fail
/// use rs_cam_core::compute::catalog::CutRule;
/// let _ = CutRule::default();
/// ```
///
/// A row rule names every field (here `axial` is missing):
///
/// ```compile_fail
/// use rs_cam_core::compute::catalog::{CutRule, RadialRule};
/// let _ = CutRule { radial: RadialRule::Slot };
/// ```
///
/// The sibling: the full literal compiles.
///
/// ```
/// use rs_cam_core::compute::catalog::{AxialRule, CutRule, RadialRule};
/// let _ = CutRule {
///     radial: RadialRule::Slot,
///     axial: AxialRule::DepthPerPass,
/// };
/// ```
///
/// Code outside `compute/` cannot make an operator value:
///
/// ```compile_fail
/// use rs_cam_core::compute::catalog::{Mm, Stated};
/// let _ = Stated::new(Mm::new(1.0));
/// ```
///
/// The sibling: code outside `compute/` makes a length, and reads the
/// operator values of a spec.
///
/// ```
/// use rs_cam_core::compute::OperationConfig;
/// use rs_cam_core::compute::catalog::{Mm, OperationType, Radial};
/// let _ = Mm::new(1.0);
/// let spec = OperationConfig::new_default(OperationType::Face).cut_spec();
/// let Radial::Width(stepover) = spec.radial else { return };
/// let _: f64 = stepover.mm();
/// ```
///
/// A total depth of `None` gives its reason:
///
/// ```compile_fail
/// use rs_cam_core::compute::catalog::TotalDepth;
/// let t: TotalDepth = TotalDepth::None;
/// ```
///
/// The sibling: with the reason it compiles.
///
/// ```
/// use rs_cam_core::compute::catalog::TotalDepth;
/// let _t: TotalDepth = TotalDepth::None("the model sets the depth");
/// ```
///
/// A cut spec has no `Default`:
///
/// ```compile_fail
/// use rs_cam_core::compute::catalog::CutSpec;
/// let _ = CutSpec::default();
/// ```
///
/// The sibling: a full literal compiles.
///
/// ```
/// use rs_cam_core::compute::catalog::{Axial, CutSpec, FirstPass, Radial, TotalDepth};
/// let _ = CutSpec {
///     radial: Radial::Slot,
///     axial: Axial::Surface { max: None },
///     total: TotalDepth::FromHeights,
///     first_pass: FirstPass::Slot,
/// };
/// ```
#[doc(hidden)]
pub mod cut_spec_refusals {}
