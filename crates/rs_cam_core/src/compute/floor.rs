//! The floor of a Pocket, Profile, Zigzag, Adaptive 2D (geometry
//! selection P3.2, ruling D9) or Bore (P6.6).
//!
//! One value names the floor and its reference. Before P3.2 the op `depth`
//! and the pinned Heights Bottom Z were two fields that could disagree, and
//! only the depth reached the cut (G-BOTTOMPIN). Now:
//!
//! - [`Floor`] is the one floor field of the floor ops
//!   (`OperationConfig::floor`).
//! - The session resolves it against the height context of the toolpath
//!   ([`Floor::resolve`] gives a [`FloorRef`]). `HeightsConfig::auto_bottom_z`
//!   reads the `FloorRef` first, so `ResolvedHeights::bottom_z` of the floor
//!   ops IS the floor.
//! - The 2.5D ladder of the four 2.5D ops runs from the top to that bottom
//!   (`ops::depth::floor_ladder`).
//! - P3.5: under [`Floor::EachFace`] each closed area of the selection is
//!   cut to its own floor: the plane Z of its face less the depth
//!   ([`region_floors`]). The resolved Bottom Z is the deepest of them
//!   ([`FloorRef::EachFace`]), and the auto top is the stock top, not the
//!   face top (ruling L-15). The prologue gives each area its own ladder
//!   (`Prepared::region_ladders`); the Layered runner steps each area down
//!   its own ladder (Pocket, Zigzag, Profile), and Adaptive 2D does the
//!   same in its own loop (P3.6, ruling L-5).
//! - A pinned Heights Bottom Z on a floor op refuses
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

use super::catalog::{ItemUse, ParamRange};
use super::config::HeightContext;
use crate::selection::{
    ItemRef, NoteCause, OpItems, RefusalCause, ResolvedSelection, SelectionNote, SelectionRefusal,
    SetupFrame,
};

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
    #[must_use]
    pub const fn value(&self) -> f64 {
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
    /// ([`FloorRef::BelowTop`]). [`Floor::EachFace`] reads `face_bottom_z`:
    /// the plane Z of the lowest up face of the selection, in the setup
    /// frame (P3.5). Every other reference gives a Z.
    ///
    /// # Errors
    ///
    /// [`FloorRefusal::NotFinite`], [`FloorRefusal::NoModelMesh`] (ruling
    /// L-4) and [`FloorRefusal::NoFace`] (each face, and no up face).
    pub fn resolve(
        &self,
        ctx: &HeightContext,
        face_bottom_z: Option<f64>,
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
            Self::EachFace { depth } => face_bottom_z
                .map(|lowest| FloorRef::EachFace {
                    deepest: lowest - depth,
                })
                .ok_or(FloorRefusal::NoFace),
            Self::Z { z } => Ok(FloorRef::At(z)),
        }
    }
}

impl Floor {
    /// The same reference with the number `value`: the depth, or the Z of
    /// [`Floor::Z`].
    #[must_use]
    pub const fn with_value(self, value: f64) -> Self {
        match self {
            Self::Top { .. } => Self::Top { depth: value },
            Self::StockTop { .. } => Self::StockTop { depth: value },
            Self::StockBottom { .. } => Self::StockBottom { depth: value },
            Self::ModelTop { .. } => Self::ModelTop { depth: value },
            Self::EachFace { .. } => Self::EachFace { depth: value },
            Self::Z { .. } => Self::Z { z: value },
        }
    }

    /// True when `self` and `other` name the same reference (the values
    /// can differ).
    #[must_use]
    pub fn same_reference(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// The Z of each floor reference of one toolpath, in the setup frame
/// (geometry selection P3.7, ruling L-10). The session builds it
/// (`ToolpathHeights::floor_references`); the GUI floor row and the height
/// diagram read it, so a change of reference and a drag use one rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloorReferences {
    /// The Top Z that [`Floor::Top`] reads: the resolved top under a floor
    /// that is not each face (an auto top follows the picked faces).
    pub top_z: f64,
    /// The stock top.
    pub stock_top_z: f64,
    /// The stock bottom.
    pub stock_bottom_z: f64,
    /// The model top, or `None` when the model has no mesh (L-4).
    pub model_top_z: Option<f64>,
    /// The plane Z of the lowest up face of the selection, or `None` with
    /// no up face. [`Floor::EachFace`] reads it: the Bottom Z under that
    /// floor is the deepest face floor.
    pub face_bottom_z: Option<f64>,
}

impl FloorReferences {
    /// The Z that the depth of `floor` is measured from, or why the
    /// reference has no Z here. [`Floor::Z`] measures from 0.
    ///
    /// # Errors
    ///
    /// [`FloorRefusal::NoModelMesh`] and [`FloorRefusal::NoFace`].
    pub fn reference_z(&self, floor: &Floor) -> Result<f64, FloorRefusal> {
        match floor {
            Floor::Top { .. } => Ok(self.top_z),
            Floor::StockTop { .. } => Ok(self.stock_top_z),
            Floor::StockBottom { .. } => Ok(self.stock_bottom_z),
            Floor::ModelTop { .. } => self.model_top_z.ok_or(FloorRefusal::NoModelMesh),
            Floor::EachFace { .. } => self.face_bottom_z.ok_or(FloorRefusal::NoFace),
            Floor::Z { .. } => Ok(0.0),
        }
    }

    /// The resolved floor Z of `floor`: its Bottom Z (under each face, the
    /// deepest face floor).
    ///
    /// # Errors
    ///
    /// As [`Self::reference_z`].
    pub fn floor_z(&self, floor: &Floor) -> Result<f64, FloorRefusal> {
        let reference = self.reference_z(floor)?;
        Ok(match floor {
            Floor::Z { z } => *z,
            Floor::Top { depth }
            | Floor::StockTop { depth }
            | Floor::StockBottom { depth }
            | Floor::ModelTop { depth }
            | Floor::EachFace { depth } => reference - depth,
        })
    }

    /// L-10: the floor with the reference of `like` whose resolved Z is
    /// `z`. A drag of the floor line writes it in the reference in force.
    ///
    /// # Errors
    ///
    /// [`FloorRefusal::NoModelMesh`] or [`FloorRefusal::NoFace`] when the
    /// reference has no Z here, and the [`Floor::door_refusal`] of the
    /// result (a floor at or above the Top Z on [`Floor::Top`]). A refused
    /// floor never reaches the config.
    pub fn at_z(&self, like: Floor, z: f64) -> Result<Floor, FloorRefusal> {
        let value = match like {
            Floor::Z { .. } => z,
            Floor::Top { .. }
            | Floor::StockTop { .. }
            | Floor::StockBottom { .. }
            | Floor::ModelTop { .. }
            | Floor::EachFace { .. } => self.reference_z(&like)? - z,
        };
        let floor = like.with_value(value);
        match floor.door_refusal() {
            Some(refusal) => Err(refusal),
            None => Ok(floor),
        }
    }

    /// L-10: `from` expressed in the reference of `to`, so that the
    /// resolved floor Z stays. When `from` has no Z here (a model top with
    /// no mesh, each face with no face), nothing resolves to keep, and the
    /// number goes across unchanged.
    ///
    /// # Errors
    ///
    /// As [`Self::at_z`].
    pub fn re_express(&self, from: Floor, to: Floor) -> Result<Floor, FloorRefusal> {
        match self.floor_z(&from) {
            Ok(z) => self.at_z(to, z),
            Err(_) => {
                self.reference_z(&to)?;
                Ok(to.with_value(from.value()))
            }
        }
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
    /// [`Floor::EachFace`] (P3.5): the deepest floor of the face regions,
    /// in the setup frame. The Bottom Z of the op is this floor; each
    /// region has its own floor ([`region_floors`]).
    EachFace {
        /// The floor of the lowest face.
        deepest: f64,
    },
}

impl FloorRef {
    /// The floor Z for the resolved `top_z`: the Bottom Z of the op.
    ///
    /// `BelowTop(d)` gives `top_z - d`: the expression of the ladder before
    /// P3.2 (`top_z - depth.abs()`) for every positive depth, so the default
    /// floor gives the same bits. A depth of zero or less puts the floor at
    /// or above the top, and the session refuses it
    /// ([`FloorRefusal::AboveTop`]). Under each face it is the deepest face
    /// floor: P3.7 skips a face whose floor is at or above the top, so the
    /// op refuses only when the deepest floor is there too (L-16).
    #[must_use]
    pub fn z(self, top_z: f64) -> f64 {
        match self {
            Self::BelowTop(depth) => top_z - depth,
            Self::At(z) => z,
            Self::EachFace { deepest } => deepest,
        }
    }

    /// True for [`FloorRef::EachFace`]: the auto top does not follow the
    /// faces (ruling L-15).
    #[must_use]
    pub const fn is_each_face(self) -> bool {
        matches!(self, Self::EachFace { .. })
    }
}

/// The floor of each closed area that an op cuts under [`Floor::EachFace`]
/// (geometry selection P3.5), and the notes of the faces it skips (P3.7).
#[derive(Debug, Clone, PartialEq)]
pub struct RegionFloors {
    /// The floor Z of each area that the op cuts, in the order of
    /// `Regions::as_slice` after the skip.
    floors: Vec<f64>,
    /// One note per skipped face (P3.7): its floor is at or above the top.
    notes: Vec<SelectionNote>,
}

impl RegionFloors {
    /// The floor Z of each area that the op cuts, in the setup frame, in
    /// the order of `Regions::as_slice` after the skip.
    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.floors
    }

    /// One note per face that the op skips because its floor is at or
    /// above the top (P3.7), in the order of the areas.
    #[must_use]
    pub fn notes(&self) -> &[SelectionNote] {
        &self.notes
    }

    /// The notes, by value.
    #[must_use]
    pub fn into_notes(self) -> Vec<SelectionNote> {
        self.notes
    }
}

/// THE per-area floor rule (geometry selection P3.5, design 5.1; P3.7 lead
/// ruling: the skip).
///
/// Under [`Floor::EachFace`] the floor of area `i` of `items` is the plane Z
/// of its exterior face (`Regions::plane_z`) less the depth. `Ok(None)` for
/// every other floor, and for items with no closed area. The prologue calls
/// it with the resolved Top Z of the op, and `selection_status` calls it
/// with the same top, so the status and generation give one answer (the
/// P2.4 rule).
///
/// P3.7: an area whose floor is at or above `top_z` has nothing to cut. The
/// rule removes it from `items` (`OpItems::skip_regions`: the face becomes
/// `ItemRole::Skipped`) and gives a [`SelectionNote`] that names the face
/// ([`NoteCause::AtTheTop`]). When no area is skipped, `items` does not
/// change.
///
/// # Errors
///
/// A [`SelectionRefusal`] that names the item:
/// [`RefusalCause::NoFaceForFloor`] when the exterior of an area is a loop,
/// a circle or the stock ring, and [`RefusalCause::FloorAboveTop`] when the
/// floor of EVERY area is at or above `top_z` (L-16; it names the first
/// face). The caller adds the op label.
pub fn region_floors(
    resolved: &ResolvedSelection<SetupFrame>,
    items: &mut OpItems,
    floor: Floor,
    top_z: f64,
) -> Result<Option<RegionFloors>, SelectionRefusal> {
    let Floor::EachFace { depth } = floor else {
        return Ok(None);
    };
    let Some(regions) = items.regions() else {
        return Ok(None);
    };
    let item_at = |index: usize| {
        regions
            .exterior_item(index)
            .and_then(|at| resolved.items().get(at))
    };
    let ref_at = |index: usize| item_at(index).map(|r| ItemRef::new(r.identity().clone()));
    let mut floors = Vec::with_capacity(regions.as_slice().len());
    let mut skipped = Vec::new();
    let mut notes = Vec::new();
    let mut first_above: Option<(usize, f64)> = None;
    for index in 0..regions.as_slice().len() {
        let Some(plane_z) = regions.plane_z(index) else {
            return Err(SelectionRefusal::new(
                ref_at(index),
                RefusalCause::NoFaceForFloor,
            ));
        };
        let floor_z = plane_z - depth;
        // A floor that is not a number refuses: it cannot be cut, and it
        // is not "at the top" either.
        if floor_z.is_nan() {
            return Err(SelectionRefusal::new(
                ref_at(index),
                RefusalCause::FloorAboveTop { floor_z, top_z },
            ));
        }
        if floor_z >= top_z {
            first_above.get_or_insert((index, floor_z));
            skipped.push(index);
            if let Some(r) = item_at(index) {
                notes.push(SelectionNote {
                    kind: r.item().kind(),
                    layer: r.item().layer().map(str::to_owned),
                    count: 1,
                    first: ItemRef::new(r.identity().clone()),
                    item_use: ItemUse::Regions,
                    cause: NoteCause::AtTheTop { floor_z, top_z },
                });
            }
            continue;
        }
        floors.push(floor_z);
    }
    // L-16: with every area skipped the ladder of the op is empty.
    if floors.is_empty()
        && let Some((index, floor_z)) = first_above
    {
        return Err(SelectionRefusal::new(
            ref_at(index),
            RefusalCause::FloorAboveTop { floor_z, top_z },
        ));
    }
    if !skipped.is_empty() {
        items.skip_regions(&skipped);
    }
    Ok(Some(RegionFloors { floors, notes }))
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

impl Floor {
    /// What the reference means: the `description` of its
    /// `get_operation_schema` entry ([`floor_schema_json`]), and the hover of
    /// the GUI floor row (P3.7). The floor is a driver-set field, so the
    /// registry holds no `help` for it; this is its one help text.
    #[must_use]
    pub const fn reference_help(&self) -> &'static str {
        reference_help(self)
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
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
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
