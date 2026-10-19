//! The floor of a Pocket, Profile, Zigzag or Adaptive 2D (geometry
//! selection P3.2, ruling D9).
//!
//! One value names the floor and its reference. Before P3.2 the op `depth`
//! and the pinned Heights Bottom Z were two fields that could disagree, and
//! only the depth reached the cut (G-BOTTOMPIN). Now:
//!
//! - [`Floor`] is the one floor field of the four ops
//!   (`OperationConfig::floor`).
//! - The session resolves it against the height context of the toolpath
//!   ([`Floor::resolve`] gives a [`FloorRef`]). `HeightsConfig::auto_bottom_z`
//!   reads the `FloorRef` first, so `ResolvedHeights::bottom_z` of the four
//!   ops IS the floor.
//! - The 2.5D ladder of the four ops runs from the top to that bottom
//!   (`ops::depth::floor_ladder`).
//! - A pinned Heights Bottom Z on the four ops refuses
//!   (`ToolpathConfig::floor_conflict`).
//!
//! Every Z is in the setup frame (the emission frame), as `HeightMode::Manual`
//! (ruling L-8).
//!
//! # The domain of the numbers (W6a)
//!
//! `floor` is a driver-set field of the op configs
//! (`#[param(driver_set = DRIVER_SET_FLOOR)]`), so the F185 compile check of
//! `#[derive(OpParams)]` does not reach inside it. The two numbers state
//! their domain here:
//!
//! - `depth` is unbounded: a signed distance below the reference (a negative
//!   value stands above it). No static bound comes from math or physics,
//!   because the valid depth depends on the reference and on the resolved
//!   top. The resolver refuses a floor at or above the top
//!   ([`FloorRefusal::AboveTop`]) and a value that is not finite.
//! - `z` is frame-dependent: an absolute Z in the setup frame, as a
//!   `frame_dependent` parameter.

use serde::{Deserialize, Serialize};

use super::config::HeightContext;

/// The floor of a Pocket, Profile, Zigzag or Adaptive 2D: one value that
/// names its reference (ruling D9). `depth` is always "mm below the
/// reference" (a negative value stands above it). Every Z is in the setup
/// frame (the emission frame).
///
/// File spelling: `floor = { from = "top", depth = 6.0 }`,
/// `floor = { from = "z", z = -20.0 }`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case", deny_unknown_fields)]
pub enum Floor {
    /// Below the resolved Top Z of the op (auto: the stock top, or the top
    /// of the picked faces; else the pinned top). The op `depth` before
    /// P3.2, and the default of each op (ruling L-3).
    Top { depth: f64 },
    /// Below the stock top.
    StockTop { depth: f64 },
    /// Below the stock bottom: a positive depth cuts into the spoilboard
    /// (ruling L-13).
    StockBottom { depth: f64 },
    /// Below the model top. A model with no mesh has no top, so this
    /// refuses there (ruling L-4).
    ModelTop { depth: f64 },
    /// Each face region of the op selection at its own plane Z (S6). Not
    /// yet: generation refuses it until P3.5.
    EachFace { depth: f64 },
    /// An absolute Z in the setup frame.
    Z { z: f64 },
}

impl Floor {
    /// The floor `depth` mm below the resolved Top Z: the default of each
    /// op.
    #[must_use]
    pub const fn top(depth: f64) -> Self {
        Self::Top { depth }
    }

    /// The name of the reference, for a message or a label.
    #[must_use]
    pub const fn reference_label(&self) -> &'static str {
        match self {
            Self::Top { .. } => "Top Z",
            Self::StockTop { .. } => "Stock top",
            Self::StockBottom { .. } => "Stock bottom",
            Self::ModelTop { .. } => "Model top",
            Self::EachFace { .. } => "Each picked face",
            Self::Z { .. } => "Absolute Z",
        }
    }

    /// The depth below the resolved Top Z, when the floor is
    /// [`Floor::Top`]. Every other reference needs the session to resolve
    /// it, so a reader with no height context gets `None` (ruling L-6).
    #[must_use]
    pub const fn depth_below_top(&self) -> Option<f64> {
        match self {
            Self::Top { depth } => Some(*depth),
            Self::StockTop { .. }
            | Self::StockBottom { .. }
            | Self::ModelTop { .. }
            | Self::EachFace { .. }
            | Self::Z { .. } => None,
        }
    }

    /// The number the floor holds: the depth, or the Z of [`Floor::Z`].
    const fn value(&self) -> f64 {
        match self {
            Self::Top { depth }
            | Self::StockTop { depth }
            | Self::StockBottom { depth }
            | Self::ModelTop { depth }
            | Self::EachFace { depth } => *depth,
            Self::Z { z } => *z,
        }
    }

    /// Resolve the floor against the height context of its toolpath.
    ///
    /// The top is not known yet when the context is built (an auto top can
    /// follow the picked faces), so [`Floor::Top`] stays relative
    /// ([`FloorRef::BelowTop`]). Every other reference gives a Z.
    ///
    /// # Errors
    ///
    /// [`FloorRefusal::NotFinite`], [`FloorRefusal::NoModelMesh`] (ruling
    /// L-4) and [`FloorRefusal::EachFaceNotYet`] (P3.5).
    pub fn resolve(&self, ctx: &HeightContext) -> Result<FloorRef, FloorRefusal> {
        if !self.value().is_finite() {
            return Err(FloorRefusal::NotFinite);
        }
        match *self {
            Self::Top { depth } => Ok(FloorRef::BelowTop(depth)),
            Self::StockTop { depth } => Ok(FloorRef::At(ctx.stock_top_z - depth)),
            Self::StockBottom { depth } => Ok(FloorRef::At(ctx.stock_bottom_z - depth)),
            Self::ModelTop { depth } => ctx
                .model_top_z
                .map(|top| FloorRef::At(top - depth))
                .ok_or(FloorRefusal::NoModelMesh),
            Self::EachFace { .. } => Err(FloorRefusal::EachFaceNotYet),
            Self::Z { z } => Ok(FloorRef::At(z)),
        }
    }
}

/// A resolved floor: what the height context carries
/// (`HeightContext::floor`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloorRef {
    /// [`Floor::Top`]: this many mm below the resolved Top Z.
    BelowTop(f64),
    /// Every other reference: the floor Z in the setup frame.
    At(f64),
}

impl FloorRef {
    /// The floor Z for the resolved `top_z`.
    ///
    /// `BelowTop(d)` gives `top_z - d`: the expression of the ladder before
    /// P3.2 (`top_z - depth.abs()`) for every positive depth, so the default
    /// floor gives the same bits. A depth of zero or less puts the floor at
    /// or above the top, and the session refuses it
    /// ([`FloorRefusal::AboveTop`]).
    #[must_use]
    pub fn z(self, top_z: f64) -> f64 {
        match self {
            Self::BelowTop(depth) => top_z - depth,
            Self::At(z) => z,
        }
    }
}

/// Why a floor does not resolve. The session puts the toolpath name in
/// front of the text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloorRefusal {
    /// The depth or the Z is not a finite number.
    NotFinite,
    /// [`Floor::ModelTop`] on a toolpath whose model has no mesh (L-4).
    NoModelMesh,
    /// [`Floor::EachFace`] before P3.5.
    EachFaceNotYet,
    /// The floor is at or above the resolved top: the ladder would be empty
    /// (ruling L-16).
    AboveTop {
        /// The resolved floor Z.
        floor_z: f64,
        /// The resolved Top Z.
        top_z: f64,
    },
}

impl std::fmt::Display for FloorRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFinite => write!(f, "the floor is not a finite number. Set a floor value"),
            Self::NoModelMesh => write!(
                f,
                "the floor reads the Model top, but the model of the toolpath has no mesh, so \
                 it has no Z extent. Set the floor from the Top Z, the stock or a Z value"
            ),
            Self::EachFaceNotYet => write!(
                f,
                "the floor \"each picked face\" is not yet available (geometry selection \
                 P3.5). Set the floor from the Top Z, the stock, the model or a Z value"
            ),
            Self::AboveTop { floor_z, top_z } => write!(
                f,
                "the floor Z {floor_z:.3} is at or above the Top Z {top_z:.3}, so the op has \
                 nothing to cut. Set a floor below the top"
            ),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, Serialize)]
    struct Holder {
        floor: Floor,
    }

    /// The file spelling of each variant reads back, and an extra key in
    /// the floor table refuses.
    #[test]
    fn the_file_spelling_round_trips() {
        let cases = [
            (
                "floor = { from = \"top\", depth = 6.0 }",
                Floor::Top { depth: 6.0 },
            ),
            (
                "floor = { from = \"stock_top\", depth = 2.0 }",
                Floor::StockTop { depth: 2.0 },
            ),
            (
                "floor = { from = \"stock_bottom\", depth = 0.5 }",
                Floor::StockBottom { depth: 0.5 },
            ),
            (
                "floor = { from = \"model_top\", depth = 1.0 }",
                Floor::ModelTop { depth: 1.0 },
            ),
            (
                "floor = { from = \"each_face\", depth = 0.0 }",
                Floor::EachFace { depth: 0.0 },
            ),
            ("floor = { from = \"z\", z = -20.0 }", Floor::Z { z: -20.0 }),
        ];
        for (text, want) in cases {
            let read: Holder = toml::from_str(text).unwrap();
            assert_eq!(read.floor, want, "{text}");
            let again: Holder = toml::from_str(&toml::to_string(&read).unwrap()).unwrap();
            assert_eq!(again.floor, want, "{text} after a write");
        }
        let extra = toml::from_str::<Holder>("floor = { from = \"top\", depth = 6.0, z = 1.0 }");
        assert!(
            extra.is_err(),
            "an extra key in the floor table must refuse"
        );
    }
}
