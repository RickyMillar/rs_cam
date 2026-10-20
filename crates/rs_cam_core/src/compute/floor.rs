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
//! - P3.5: under [`Floor::EachFace`] each closed area of the selection is
//!   cut to its own floor: the plane Z of its face less the depth
//!   ([`region_floors`]). The resolved Bottom Z is the deepest of them
//!   ([`FloorRef::EachFace`]), and the auto top is the stock top, not the
//!   face top (ruling L-15). The prologue gives each area its own ladder
//!   (`Prepared::region_ladders`); the Layered runner steps each area down
//!   its own ladder (Pocket, Zigzag, Profile), and Adaptive 2D does the
//!   same in its own loop (P3.6, ruling L-5).
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
//! `#[derive(OpParams)]` does not reach inside it. [`Floor::domain`] states
//! the domain of each number in code, and two readers use it (P3.3): the
//! `set_toolpath_floor` door ([`Floor::door_refusal`]) and the
//! `get_operation_schema` entry ([`floor_schema_json`]).
//!
//! - `depth` of [`Floor::Top`] is more than 0: a depth of 0 or less puts the
//!   floor at or above the top by construction (L-16).
//! - `depth` of every other reference is unbounded: a signed distance below
//!   the reference (a negative value stands above it, for example an onion
//!   skin above the stock bottom, L-13). The valid depth depends on the
//!   resolved top, so the session refuses a floor at or above the top
//!   ([`FloorRefusal::AboveTop`]) at generation.
//! - `z` is frame-dependent: an absolute Z in the setup frame, as a
//!   `frame_dependent` parameter.
//! - Every number is finite.

use serde::{Deserialize, Serialize};

use super::catalog::ParamRange;
use super::config::HeightContext;
use crate::selection::{ItemRef, RefusalCause, Regions, ResolvedSelection, SelectionRefusal};

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
    /// Each face region of the op selection at its own plane Z less the
    /// depth (S6). Pocket, Zigzag and Profile cut it (P3.5), and Adaptive
    /// 2D (P3.6).
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

    /// One floor of each reference, in the order of the enum. The
    /// `get_operation_schema` entry lists them ([`floor_schema_json`]).
    pub const EACH_REFERENCE: [Self; 6] = [
        Self::Top { depth: 0.0 },
        Self::StockTop { depth: 0.0 },
        Self::StockBottom { depth: 0.0 },
        Self::ModelTop { depth: 0.0 },
        Self::EachFace { depth: 0.0 },
        Self::Z { z: 0.0 },
    ];

    /// The `from` token of the reference: the file and wire spelling.
    #[must_use]
    pub const fn from_token(&self) -> &'static str {
        match self {
            Self::Top { .. } => "top",
            Self::StockTop { .. } => "stock_top",
            Self::StockBottom { .. } => "stock_bottom",
            Self::ModelTop { .. } => "model_top",
            Self::EachFace { .. } => "each_face",
            Self::Z { .. } => "z",
        }
    }

    /// The name of the number the floor holds: `depth`, or `z` for
    /// [`Floor::Z`].
    #[must_use]
    pub const fn value_name(&self) -> &'static str {
        match self {
            Self::Top { .. }
            | Self::StockTop { .. }
            | Self::StockBottom { .. }
            | Self::ModelTop { .. }
            | Self::EachFace { .. } => "depth",
            Self::Z { .. } => "z",
        }
    }

    /// The domain of the number the floor holds (W6a; see the module
    /// documentation). The door and the schema read this one value.
    #[must_use]
    pub const fn domain(&self) -> ParamRange {
        match self {
            Self::Top { .. } => ParamRange {
                min: Some(0.0),
                max: None,
                min_exclusive: true,
            },
            Self::StockTop { .. }
            | Self::StockBottom { .. }
            | Self::ModelTop { .. }
            | Self::EachFace { .. }
            | Self::Z { .. } => ParamRange {
                min: None,
                max: None,
                min_exclusive: false,
            },
        }
    }

    /// The refusal of the floor that is known with no model and no height
    /// context: the `set_toolpath_floor` door (P3.3). A number that is not
    /// finite, and a number outside [`Self::domain`]. A floor at or above a
    /// resolved top that is not [`Floor::Top`] needs the session, so
    /// generation refuses it.
    #[must_use]
    pub fn door_refusal(&self) -> Option<FloorRefusal> {
        let value = self.value();
        if !value.is_finite() {
            return Some(FloorRefusal::NotFinite);
        }
        (!self.domain().accepts(value)).then_some(FloorRefusal::OutOfDomain {
            from: self.from_token(),
            name: self.value_name(),
            value,
        })
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
    /// ([`FloorRef::BelowTop`]). [`Floor::EachFace`] reads `faces`: the
    /// plane Z of the highest and of the lowest up face of the selection,
    /// in the setup frame (P3.5). Every other reference gives a Z.
    ///
    /// # Errors
    ///
    /// [`FloorRefusal::NotFinite`], [`FloorRefusal::NoModelMesh`] (ruling
    /// L-4) and [`FloorRefusal::NoFace`] (each face, and no up face).
    pub fn resolve(
        &self,
        ctx: &HeightContext,
        faces: Option<FacePlanes>,
    ) -> Result<FloorRef, FloorRefusal> {
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
            Self::EachFace { depth } => faces
                .map(|faces| FloorRef::EachFace {
                    highest: faces.highest - depth,
                    deepest: faces.lowest - depth,
                })
                .ok_or(FloorRefusal::NoFace),
            Self::Z { z } => Ok(FloorRef::At(z)),
        }
    }
}

/// The plane Z of the highest and of the lowest up face of a selection, in
/// the setup frame (`ResolvedSelection::face_top_z`, `face_bottom_z`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FacePlanes {
    /// The highest plane Z.
    pub highest: f64,
    /// The lowest plane Z.
    pub lowest: f64,
}

impl FacePlanes {
    /// The planes from the face top and the face bottom of a selection, or
    /// `None` when it holds no up face.
    #[must_use]
    pub fn from_top_and_bottom(top: Option<f64>, bottom: Option<f64>) -> Option<Self> {
        Some(Self {
            highest: top?,
            lowest: bottom?,
        })
    }
}

/// A resolved floor: what the height context carries
/// (`HeightContext::floor`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloorRef {
    /// [`Floor::Top`]: this many mm below the resolved Top Z.
    BelowTop(f64),
    /// Every other reference except each face: the floor Z in the setup
    /// frame.
    At(f64),
    /// [`Floor::EachFace`] (P3.5): the highest and the deepest floor of the
    /// face regions, in the setup frame. The Bottom Z of the op is the
    /// deepest; each region has its own floor ([`region_floors`]).
    EachFace {
        /// The floor of the highest face.
        highest: f64,
        /// The floor of the lowest face.
        deepest: f64,
    },
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
            Self::EachFace { deepest, .. } => deepest,
        }
    }

    /// The highest floor Z for the resolved `top_z`: the floor that meets
    /// the top first. It equals [`Self::z`] for every reference except
    /// each face, whose highest face floor is the one to check (L-16).
    #[must_use]
    pub fn highest_z(self, top_z: f64) -> f64 {
        match self {
            Self::BelowTop(_) | Self::At(_) => self.z(top_z),
            Self::EachFace { highest, .. } => highest,
        }
    }

    /// True for [`FloorRef::EachFace`]: the auto top does not follow the
    /// faces (ruling L-15).
    #[must_use]
    pub const fn is_each_face(self) -> bool {
        matches!(self, Self::EachFace { .. })
    }
}

/// The floor of each closed area of an op under [`Floor::EachFace`], in
/// the order of `Regions::as_slice` (geometry selection P3.5).
#[derive(Debug, Clone, PartialEq)]
pub struct RegionFloors(Vec<f64>);

impl RegionFloors {
    /// The floor Z of each area, in the setup frame.
    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.0
    }
}

/// THE per-area floor rule (geometry selection P3.5, design 5.1).
///
/// Under [`Floor::EachFace`] the floor of area `i` is the plane Z of its
/// exterior face (`Regions::plane_z`) less the depth. `None` for every
/// other floor. The prologue calls it with the resolved Top Z of the op,
/// and `selection_status` calls it with the same top, so the status and
/// generation give one refusal (the P2.4 rule).
///
/// # Errors
///
/// A [`SelectionRefusal`] that names the item:
/// [`RefusalCause::NoFaceForFloor`] when the exterior of an area is a loop,
/// a circle or the stock ring, and [`RefusalCause::FloorAboveTop`] when the
/// floor of an area is at or above `top_z` (L-16). The caller adds the op
/// label.
pub fn region_floors(
    resolved: &ResolvedSelection,
    regions: &Regions,
    floor: Floor,
    top_z: f64,
) -> Result<Option<RegionFloors>, SelectionRefusal> {
    let Floor::EachFace { depth } = floor else {
        return Ok(None);
    };
    let item_at = |index: usize| {
        regions
            .exterior_item(index)
            .and_then(|at| resolved.items().get(at))
            .map(|r| ItemRef::new(r.identity().clone()))
    };
    let mut floors = Vec::with_capacity(regions.as_slice().len());
    for index in 0..regions.as_slice().len() {
        let Some(plane_z) = regions.plane_z(index) else {
            return Err(SelectionRefusal::new(
                item_at(index),
                RefusalCause::NoFaceForFloor,
            ));
        };
        let floor_z = plane_z - depth;
        // A floor that is not a number refuses too.
        if floor_z >= top_z || floor_z.is_nan() {
            return Err(SelectionRefusal::new(
                item_at(index),
                RefusalCause::FloorAboveTop { floor_z, top_z },
            ));
        }
        floors.push(floor_z);
    }
    Ok(Some(RegionFloors(floors)))
}

/// Why a floor does not resolve. The session puts the toolpath name in
/// front of the text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloorRefusal {
    /// The depth or the Z is not a finite number.
    NotFinite,
    /// [`Floor::ModelTop`] on a toolpath whose model has no mesh (L-4).
    NoModelMesh,
    /// [`Floor::EachFace`] on a selection with no up face (P3.5). The
    /// prologue refuses the area by name (`RefusalCause::NoFaceForFloor`).
    NoFace,
    /// The number is outside [`Floor::domain`] (P3.3): a depth of 0 or
    /// less below the Top Z.
    OutOfDomain {
        /// The `from` token of the reference.
        from: &'static str,
        /// The name of the number (`depth` or `z`).
        name: &'static str,
        /// The value.
        value: f64,
    },
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
            Self::NoFace => write!(
                f,
                "the floor is \"each picked face\", but the selection holds no up face. Pick \
                 faces, or set the floor from the Top Z, the stock, the model or a Z value"
            ),
            Self::OutOfDomain { from, name, value } => write!(
                f,
                "the floor {{ from = \"{from}\", {name} = {value} }} is at or above the top, so \
                 the op has nothing to cut. Set a {name} more than 0"
            ),
            Self::AboveTop { floor_z, top_z } => write!(
                f,
                "the floor Z {floor_z:.3} is at or above the Top Z {top_z:.3}, so the op has \
                 nothing to cut. Set a floor below the top"
            ),
        }
    }
}

/// What a reference means, for the `get_operation_schema` entry.
const fn reference_help(floor: &Floor) -> &'static str {
    match floor {
        Floor::Top { .. } => {
            "mm below the resolved Top Z of the op (auto: the stock top, or the top of the \
             picked faces; else the pinned top). The default"
        }
        Floor::StockTop { .. } => "mm below the stock top; a negative value stands above it",
        Floor::StockBottom { .. } => {
            "mm below the stock bottom; a positive value cuts into the spoilboard, a negative \
             value leaves a skin"
        }
        Floor::ModelTop { .. } => {
            "mm below the model top; a model with no mesh has no top, so generation refuses it"
        }
        Floor::EachFace { .. } => {
            "mm below the plane Z of each picked face: each face region is cut to its own \
             floor, from the stock top (or the pinned top). Every closed area of the selection \
             must be a face"
        }
        Floor::Z { .. } => "an absolute Z in the setup frame (frame-dependent)",
    }
}

/// The `floor` entry of `get_operation_schema` for an op whose default
/// floor is `default` (P3.3). It lists each reference with the name and the
/// domain of its number ([`Floor::domain`]), and names the tool that writes
/// the floor. The JSON of a floor is the file JSON.
#[must_use]
pub fn floor_schema_json(default: Floor) -> serde_json::Value {
    let variants: Vec<serde_json::Value> = Floor::EACH_REFERENCE
        .iter()
        .map(|floor| {
            serde_json::json!({
                "from": floor.from_token(),
                "label": floor.reference_label(),
                "field": floor.value_name(),
                "range": floor.domain().to_json(),
                "frame_dependent": matches!(floor, Floor::Z { .. }),
                "description": reference_help(floor),
            })
        })
        .collect();
    serde_json::json!({
        "set_through": "set_toolpath_floor",
        "note": "The floor names its reference. Write it with set_toolpath_floor \
                 { index, floor }; set_toolpath_param refuses it. A floor at or above the \
                 resolved top refuses at generation.",
        "default": default,
        "variants": variants,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
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

    /// P3.3: the door refusal reads the domain. A Top depth of 0 or less
    /// refuses; a negative depth on another reference passes; a number that
    /// is not finite refuses. P3.5: Each Face passes the door. The `from`
    /// token is the serde token.
    #[test]
    fn the_door_reads_the_domain() {
        assert!(Floor::Top { depth: 6.0 }.door_refusal().is_none());
        assert!(matches!(
            Floor::Top { depth: 0.0 }.door_refusal(),
            Some(FloorRefusal::OutOfDomain { .. })
        ));
        assert!(Floor::StockBottom { depth: -1.0 }.door_refusal().is_none());
        assert!(Floor::Z { z: -20.0 }.door_refusal().is_none());
        assert_eq!(
            Floor::Z { z: f64::NAN }.door_refusal(),
            Some(FloorRefusal::NotFinite)
        );
        assert_eq!(Floor::EachFace { depth: 1.0 }.door_refusal(), None);
        assert_eq!(Floor::EachFace { depth: -1.0 }.door_refusal(), None);
        for floor in Floor::EACH_REFERENCE {
            let json = serde_json::to_value(floor).unwrap();
            assert_eq!(json["from"], serde_json::json!(floor.from_token()));
            assert!(json.get(floor.value_name()).is_some(), "{json}");
        }
    }
}
