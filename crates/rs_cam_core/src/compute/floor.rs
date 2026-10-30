//! The floor of a Pocket, Profile, Zigzag, Adaptive 2D (geometry
//! selection P3.2, ruling D9), Bore (P6.6) or Drill (R-B, F19).
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
//! - R-B, F19: a Drill and a Bore cut each hole to its own floor
//!   ([`hole_floors`], one [`HoleSpan`] per hole): [`Floor::Top`] counts the
//!   depth from the rim of each hole, [`Floor::EachHole`] goes to the floor
//!   of each hole. The Bottom Z is the deepest hole floor
//!   ([`FloorRef::PerHole`]).
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
use crate::selection::HoleFitRule;
use crate::selection::{
    ItemKind, ItemRef, MatchKey, NoteCause, OpItems, RefusalCause, ResolvedSelection,
    SelectionNote, SelectionRefusal, SetupFrame, SourceRef,
};

/// The floor of a Pocket, Profile, Zigzag or Adaptive 2D: one value that
/// names its reference (ruling D9). `depth` is always "mm below the
/// reference" (a negative value stands above it). Every Z is in the setup
/// frame (the emission frame).
///
/// File spelling: `floor = { from = "top", depth = 6.0 }`,
/// `floor = { from = "z", z = -20.0 }`,
/// `floor = { from = "face", depth = 0.0, key = { .. } }` (UX-8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case", deny_unknown_fields)]
pub enum Floor {
    /// Below the resolved Top Z of the op (auto: the stock top; else the
    /// pinned top). Under an auto top and a face pick the depth measures
    /// from the highest picked face, and the Top Z is the higher of that
    /// face and the stock top (UX-1), so the Top Z can read higher than
    /// the floor plus the depth. The op `depth` before P3.2, and the
    /// default of each op (ruling L-3).
    Top { depth: f64 },
    /// Below the stock top.
    StockTop { depth: f64 },
    /// Below the stock bottom: a positive depth cuts into the spoilboard
    /// (ruling L-13).
    StockBottom { depth: f64 },
    /// Below the model top. A model with no mesh has no top, so this
    /// refuses there (ruling L-4).
    ModelTop { depth: f64 },
    /// Below the model bottom: the lowest Z of the model in the setup
    /// frame (D1, Fusion "Model bottom"). A positive depth goes further
    /// down, a negative depth stands above it. A model with no mesh (a
    /// drawing) has no bottom, so this refuses there (ruling L-4). No rule
    /// clamps it at the stock bottom, as for [`Floor::ModelTop`]; the
    /// depth-beyond-stock caution names a floor below the stock bottom.
    ModelBottom { depth: f64 },
    /// Each face region of the op selection at its own plane Z less the
    /// depth (S6). Pocket, Zigzag and Profile cut it (P3.5), and Adaptive
    /// 2D (P3.6).
    EachFace { depth: f64 },
    /// An absolute Z in the setup frame.
    Z { z: f64 },
    /// A picked face (geometry selection UX-8, operator ruling Q4): the
    /// plane Z of the one item that `key` names, in the setup frame, less
    /// the depth. The item is an up `Face` or a model ring (a `Loop` with
    /// a Z). The key follows a re-import as a pick does. A key that names
    /// no item refuses generation ([`FloorRefusal::FaceLost`]).
    Face { key: MatchKey, depth: f64 },
    /// R-B (operator ruling 2026-10-23): each hole of a STEP model to its
    /// own floor, less the depth. A blind hole goes to its floor circle; a
    /// through hole goes to the model bottom, and never below the stock
    /// bottom (P7 ruling O-2). A positive depth goes deeper. The holes of
    /// the op ([`hole_floors`]) read it: a Drill and a Bore.
    EachHole { depth: f64 },
    /// S11 (operator ruling 2026-10-23): no floor is set. No rule gives a
    /// floor for a drawing, so a new op has none, and generation refuses
    /// ([`FloorRefusal::Unset`]) until the operator sets one. A file with
    /// no `floor` key loads this, and the file never writes it.
    Unset,
}

impl Default for Floor {
    /// No floor (S11): the registry default of each floor op.
    fn default() -> Self {
        Self::Unset
    }
}

impl Floor {
    /// The floor `depth` mm below the resolved Top Z.
    #[must_use]
    pub const fn top(depth: f64) -> Self {
        Self::Top { depth }
    }

    /// True when no floor is set (S11). The serde `skip_serializing_if`
    /// of each floor field reads it.
    #[must_use]
    pub const fn is_unset(&self) -> bool {
        matches!(self, Self::Unset)
    }

    /// The name of the reference, for a message or a label.
    #[must_use]
    pub const fn reference_label(&self) -> &'static str {
        match self {
            Self::Top { .. } => "Top height",
            Self::StockTop { .. } => "Stock top",
            Self::StockBottom { .. } => "Stock bottom",
            Self::ModelTop { .. } => "Model top",
            Self::ModelBottom { .. } => "Model bottom",
            Self::EachFace { .. } => "Each picked face",
            Self::Z { .. } => "Absolute Z",
            Self::Face { .. } => "A picked face",
            Self::EachHole { .. } => "Each hole floor",
            Self::Unset => "Not set",
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
            | Self::ModelBottom { .. }
            | Self::EachFace { .. }
            | Self::Z { .. }
            | Self::Face { .. }
            | Self::EachHole { .. }
            | Self::Unset => None,
        }
    }

    /// One floor of each reference, in the order of the enum. The
    /// `get_operation_schema` entry lists them ([`floor_schema_json`]).
    /// [`Floor::Face`] holds a placeholder key: only its reference, its
    /// number and its domain are read.
    pub const EACH_REFERENCE: [Self; 9] = [
        Self::Top { depth: 0.0 },
        Self::StockTop { depth: 0.0 },
        Self::StockBottom { depth: 0.0 },
        Self::ModelTop { depth: 0.0 },
        Self::ModelBottom { depth: 0.0 },
        Self::EachFace { depth: 0.0 },
        Self::Z { z: 0.0 },
        Self::Face {
            key: MatchKey {
                source: SourceRef::ToolpathModel,
                kind: ItemKind::Face,
                layer: None,
                anchor: [0.0, 0.0],
                measure: 0.0,
                brep: None,
                z: None,
            },
            depth: 0.0,
        },
        Self::EachHole { depth: 0.0 },
    ];

    /// The references that the GUI floor row of an op of `item_use` offers
    /// ([`Self::admits`]). [`Floor::Face`] is not one: "Use as floor" in
    /// the Geometry tab is its one GUI door (L-R5).
    pub fn row_references(item_use: ItemUse) -> impl Iterator<Item = Self> {
        Self::EACH_REFERENCE
            .into_iter()
            .filter(move |floor| !matches!(floor, Self::Face { .. }) && floor.admits(item_use))
    }

    /// True when an op of `item_use` can cut to this floor (R-B).
    ///
    /// - [`Floor::EachHole`] reads the holes of the op: a Drill or a Bore
    ///   ([`ItemUse::Holes`]).
    /// - A Drill ([`HoleFitRule::ToolSize`]) takes no face floor
    ///   ([`Floor::EachFace`], [`Floor::Face`]): a hole has no face.
    /// - Every other reference serves every op.
    #[must_use]
    pub const fn admits(&self, item_use: ItemUse) -> bool {
        match self {
            Self::EachHole { .. } => matches!(item_use, ItemUse::Holes { .. }),
            Self::EachFace { .. } | Self::Face { .. } => !matches!(
                item_use,
                ItemUse::Holes {
                    fit: HoleFitRule::ToolSize,
                    ..
                }
            ),
            Self::Top { .. }
            | Self::StockTop { .. }
            | Self::StockBottom { .. }
            | Self::ModelTop { .. }
            | Self::ModelBottom { .. }
            | Self::Z { .. }
            | Self::Unset => true,
        }
    }

    /// The refusal of this floor on an op of `item_use`
    /// ([`Self::admits`]), or `None`.
    #[must_use]
    pub const fn op_refusal(&self, item_use: ItemUse) -> Option<FloorRefusal> {
        if self.admits(item_use) {
            None
        } else {
            Some(FloorRefusal::NotForThisOp {
                from: self.from_token(),
            })
        }
    }

    /// The face key of [`Floor::Face`], else `None`.
    #[must_use]
    pub const fn face_key(&self) -> Option<&MatchKey> {
        match self {
            Self::Face { key, .. } => Some(key),
            Self::Top { .. }
            | Self::StockTop { .. }
            | Self::StockBottom { .. }
            | Self::ModelTop { .. }
            | Self::ModelBottom { .. }
            | Self::EachFace { .. }
            | Self::Z { .. }
            | Self::EachHole { .. }
            | Self::Unset => None,
        }
    }

    /// The `from` token of the reference: the file and wire spelling.
    #[must_use]
    pub const fn from_token(&self) -> &'static str {
        match self {
            Self::Top { .. } => "top",
            Self::StockTop { .. } => "stock_top",
            Self::StockBottom { .. } => "stock_bottom",
            Self::ModelTop { .. } => "model_top",
            Self::ModelBottom { .. } => "model_bottom",
            Self::EachFace { .. } => "each_face",
            Self::Z { .. } => "z",
            Self::Face { .. } => "face",
            Self::EachHole { .. } => "each_hole",
            Self::Unset => "unset",
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
            | Self::ModelBottom { .. }
            | Self::EachFace { .. }
            | Self::Face { .. }
            | Self::EachHole { .. }
            | Self::Unset => "depth",
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
            | Self::ModelBottom { .. }
            | Self::EachFace { .. }
            | Self::Z { .. }
            | Self::Face { .. }
            | Self::EachHole { .. }
            | Self::Unset => ParamRange {
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
    /// generation refuses it. [`Floor::Unset`] passes the door: it clears
    /// the floor, and generation then refuses (S11).
    #[must_use]
    pub fn door_refusal(&self) -> Option<FloorRefusal> {
        let value = self.value()?;
        if !value.is_finite() {
            return Some(FloorRefusal::NotFinite);
        }
        if let Self::Face { key, .. } = self
            && !face_key_is_a_floor(key)
        {
            return Some(FloorRefusal::NotAFloorItem { kind: key.kind });
        }
        (!self.domain().accepts(value)).then_some(FloorRefusal::OutOfDomain {
            from: self.from_token(),
            name: self.value_name(),
            value,
        })
    }

    /// The number the floor holds: the depth, or the Z of [`Floor::Z`].
    /// `None` for [`Floor::Unset`]: it holds no number.
    #[must_use]
    pub const fn value(&self) -> Option<f64> {
        match self {
            Self::Top { depth }
            | Self::StockTop { depth }
            | Self::StockBottom { depth }
            | Self::ModelTop { depth }
            | Self::ModelBottom { depth }
            | Self::EachFace { depth }
            | Self::Face { depth, .. }
            | Self::EachHole { depth } => Some(*depth),
            Self::Z { z } => Some(*z),
            Self::Unset => None,
        }
    }

    /// Resolve the floor against the height context of its toolpath.
    ///
    /// The top is not known yet when the context is built (an auto top can
    /// follow the picked faces), so [`Floor::Top`] stays relative
    /// ([`FloorRef::BelowTop`]). [`Floor::EachFace`] reads `face_bottom_z`:
    /// the plane Z of the lowest up face of the selection, in the setup
    /// frame (P3.5). [`Floor::Face`] reads `floor_face_z`: the plane Z of
    /// its face, in the setup frame (UX-8; the session resolves the key).
    /// [`Floor::EachHole`] reads `hole_floor_z`: the deepest hole floor of
    /// the selection, in the setup frame (R-B; the session reads it from
    /// [`hole_floors`]). Every other reference gives a Z.
    ///
    /// # Errors
    ///
    /// [`FloorRefusal::NotFinite`], [`FloorRefusal::NoModelMesh`] (ruling
    /// L-4), [`FloorRefusal::NoFace`] (each face, and no up face),
    /// [`FloorRefusal::FaceLost`] (a face key that resolves to no plane),
    /// [`FloorRefusal::NoHoleDepth`] (each hole, and no hole floor) and
    /// [`FloorRefusal::Unset`] (no floor is set, S11).
    pub fn resolve(
        &self,
        ctx: &HeightContext,
        face_bottom_z: Option<f64>,
        floor_face_z: Option<f64>,
        hole_floor_z: Option<f64>,
    ) -> Result<FloorRef, FloorRefusal> {
        let Some(value) = self.value() else {
            return Err(FloorRefusal::Unset);
        };
        if !value.is_finite() {
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
            Self::ModelBottom { depth } => ctx
                .model_bottom_z
                .map(|bottom| FloorRef::At(bottom - depth))
                .ok_or(FloorRefusal::NoModelMesh),
            Self::EachFace { depth } => face_bottom_z
                .map(|lowest| FloorRef::EachFace {
                    deepest: lowest - depth,
                })
                .ok_or(FloorRefusal::NoFace),
            Self::Z { z } => Ok(FloorRef::At(z)),
            Self::Face { depth, .. } => floor_face_z
                .map(|z| FloorRef::At(z - depth))
                .ok_or(FloorRefusal::FaceLost),
            // The extra depth is in the hole floors already (`hole_floors`).
            Self::EachHole { .. } => hole_floor_z
                .map(|deepest| FloorRef::PerHole { deepest })
                .ok_or(FloorRefusal::NoHoleDepth),
            Self::Unset => Err(FloorRefusal::Unset),
        }
    }
}

/// UX-8: true when `key` can name a floor face: a `Face`, or a model ring
/// (a `Loop` with a Z, UX-7). A drawing loop has no plane Z, and a path, a
/// point or a circle is not an area.
#[must_use]
pub fn face_key_is_a_floor(key: &MatchKey) -> bool {
    match key.kind {
        ItemKind::Face => true,
        ItemKind::Loop => key.z.is_some(),
        ItemKind::Path | ItemKind::Point | ItemKind::Circle => false,
    }
}

impl Floor {
    /// The same reference with the number `value`: the depth, or the Z of
    /// [`Floor::Z`]. [`Floor::Unset`] has no reference, so a number typed
    /// into an empty floor gives [`Floor::Top`] (the GUI floor row).
    #[must_use]
    pub fn with_value(self, value: f64) -> Self {
        match self {
            Self::Top { .. } => Self::Top { depth: value },
            Self::StockTop { .. } => Self::StockTop { depth: value },
            Self::StockBottom { .. } => Self::StockBottom { depth: value },
            Self::ModelTop { .. } => Self::ModelTop { depth: value },
            Self::ModelBottom { .. } => Self::ModelBottom { depth: value },
            Self::EachFace { .. } => Self::EachFace { depth: value },
            Self::Z { .. } => Self::Z { z: value },
            Self::Face { key, .. } => Self::Face { key, depth: value },
            Self::EachHole { .. } => Self::EachHole { depth: value },
            Self::Unset => Self::Top { depth: value },
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
    /// The Z that [`Floor::Top`] reads: the resolved top under a floor
    /// that is not each face, or under an auto top the highest picked face
    /// (UX-1: the resolved top is then the higher of it and the stock top).
    pub top_z: f64,
    /// The stock top.
    pub stock_top_z: f64,
    /// The stock bottom.
    pub stock_bottom_z: f64,
    /// The model top, or `None` when the model has no mesh (L-4).
    pub model_top_z: Option<f64>,
    /// The model bottom, or `None` when the model has no mesh (L-4, D1).
    pub model_bottom_z: Option<f64>,
    /// The plane Z of the lowest up face of the selection, or `None` with
    /// no up face. [`Floor::EachFace`] reads it: the Bottom Z under that
    /// floor is the deepest face floor.
    pub face_bottom_z: Option<f64>,
    /// The plane Z of the face of [`Floor::Face`] (UX-8), or `None` when
    /// the floor is not a face or its key resolves to no plane.
    pub floor_face_z: Option<f64>,
    /// R-B: the deepest hole floor of the selection with no extra depth
    /// (blind: the floor circle; through: the model bottom, not below the
    /// stock bottom), or `None` when no hole has a depth.
    /// [`Floor::EachHole`] reads it.
    pub hole_floor_z: Option<f64>,
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
            Floor::ModelBottom { .. } => self.model_bottom_z.ok_or(FloorRefusal::NoModelMesh),
            Floor::EachFace { .. } => self.face_bottom_z.ok_or(FloorRefusal::NoFace),
            Floor::Z { .. } => Ok(0.0),
            Floor::Face { .. } => self.floor_face_z.ok_or(FloorRefusal::FaceLost),
            Floor::EachHole { .. } => self.hole_floor_z.ok_or(FloorRefusal::NoHoleDepth),
            Floor::Unset => Err(FloorRefusal::Unset),
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
            | Floor::ModelBottom { depth }
            | Floor::EachFace { depth }
            | Floor::Face { depth, .. }
            | Floor::EachHole { depth } => reference - depth,
            // `reference_z` refused it above.
            Floor::Unset => return Err(FloorRefusal::Unset),
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
        let value = match &like {
            Floor::Z { .. } => z,
            Floor::Top { .. }
            | Floor::StockTop { .. }
            | Floor::StockBottom { .. }
            | Floor::ModelTop { .. }
            | Floor::ModelBottom { .. }
            | Floor::EachFace { .. }
            | Floor::Face { .. }
            | Floor::EachHole { .. }
            | Floor::Unset => self.reference_z(&like)? - z,
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
    pub fn re_express(&self, from: &Floor, to: Floor) -> Result<Floor, FloorRefusal> {
        match self.floor_z(from) {
            Ok(z) => self.at_z(to, z),
            Err(_) => {
                self.reference_z(&to)?;
                Ok(match from.value() {
                    Some(value) => to.with_value(value),
                    None => to,
                })
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
    /// R-B: each hole of a Drill or a Bore to its own floor
    /// ([`hole_floors`]): [`Floor::EachHole`], and [`Floor::Top`] on holes
    /// whose rims are at more than one Z. The Bottom Z of the op is the
    /// deepest hole floor.
    PerHole {
        /// The floor of the deepest hole.
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
            Self::EachFace { deepest } | Self::PerHole { deepest } => deepest,
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
    floor: &Floor,
    top_z: f64,
) -> Result<Option<RegionFloors>, SelectionRefusal> {
    let Floor::EachFace { depth } = *floor else {
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

/// The heights that the per-hole rule reads ([`hole_floors`]), in the
/// setup frame. The session builds it from `ToolpathHeights` and the
/// prologue from `ResolvedHeights`, so both give one answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoleFloorInputs {
    /// The resolved Top Z: the cut starts at or below it.
    pub top_z: f64,
    /// True when the Top Z is pinned (Manual or from a reference): each
    /// hole then measures its depth from it, not from its rim.
    pub top_pinned: bool,
    /// The Z a depth measures from when a hole has no rim
    /// (`ResolvedHeights::depth_base_z`): a point or a drawing circle.
    pub depth_base_z: f64,
    /// The stock bottom: a through hole does not go below it (O-2).
    pub stock_bottom_z: f64,
    /// The resolved Bottom Z of a floor that is one Z for every hole (the
    /// stock, the model, an absolute Z).
    pub bottom_z: f64,
}

impl HoleFloorInputs {
    /// The inputs from the resolved heights of the op and the stock bottom
    /// (both in the setup frame).
    #[must_use]
    pub const fn new(heights: &super::config::ResolvedHeights, stock_bottom_z: f64) -> Self {
        Self {
            top_z: heights.top_z,
            top_pinned: heights.top_pinned,
            depth_base_z: heights.depth_base_z,
            stock_bottom_z,
            bottom_z: heights.bottom_z,
        }
    }
}

/// The depth base and the floor of one hole (R-B), in the setup frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoleSpan {
    /// The Z the depth of the hole measures from: its rim, or the pinned
    /// Top Z, or the depth base of the op (a hole with no rim).
    pub base_z: f64,
    /// The floor of the hole: where the tool tip stops.
    pub floor_z: f64,
}

/// R-B: the floor of the hole of depth `depth` with its rim at `rim_z`,
/// with no extra depth: a blind hole to its floor circle, a through hole
/// to the model bottom and never below `stock_bottom_z` (O-2).
#[must_use]
pub fn hole_floor_z(rim_z: f64, depth: crate::selection::HoleDepth, stock_bottom_z: f64) -> f64 {
    match depth {
        crate::selection::HoleDepth::Blind { depth_mm } => rim_z - depth_mm,
        crate::selection::HoleDepth::Through { depth_mm } => (rim_z - depth_mm).max(stock_bottom_z),
    }
}

/// THE per-hole floor rule (R-B, F19). One [`HoleSpan`] per hole of
/// `holes` (the fitted holes of a Drill or a Bore), in their order.
///
/// - The depth base of a hole is the pinned Top Z, else its rim
///   (`Hole::rim_z`), else `inputs.depth_base_z` (a hole with no rim: a
///   point or a circle of a drawing, which gives the old numbers).
/// - [`Floor::Top`]: the base less the depth. A typed depth counts from
///   the rim of each hole.
/// - [`Floor::EachHole`]: the floor of the hole ([`hole_floor_z`]) less
///   the depth.
/// - Every other reference: `inputs.bottom_z` for each hole.
///
/// The prologue calls it, and `selection_status` calls it with the same
/// inputs, so the status and generation give one answer (the P2.4 rule).
///
/// # Errors
///
/// A [`SelectionRefusal`] that names the hole: [`RefusalCause::NoHoleDepth`]
/// under [`Floor::EachHole`] for a hole with no depth, and
/// [`RefusalCause::FloorAboveTop`] for a floor at or above the Top Z (L-16).
pub fn hole_floors(
    resolved: &ResolvedSelection<SetupFrame>,
    holes: &crate::selection::Holes,
    floor: &Floor,
    inputs: &HoleFloorInputs,
) -> Result<Vec<HoleSpan>, SelectionRefusal> {
    let ref_at = |index: usize| {
        holes
            .item(index)
            .and_then(|at| resolved.items().get(at))
            .map(|r| ItemRef::new(r.identity().clone()))
    };
    let mut spans = Vec::with_capacity(holes.as_slice().len());
    for (index, hole) in holes.as_slice().iter().enumerate() {
        let base_z = match (inputs.top_pinned, hole.rim_z) {
            (true, _) => inputs.top_z,
            (false, Some(rim)) => rim,
            (false, None) => inputs.depth_base_z,
        };
        let floor_z = match *floor {
            Floor::Top { depth } => base_z - depth,
            Floor::EachHole { depth } => match (hole.rim_z, hole.depth) {
                (Some(rim), Some(hole_depth)) => {
                    hole_floor_z(rim, hole_depth, inputs.stock_bottom_z) - depth
                }
                (None, _) | (_, None) => {
                    return Err(SelectionRefusal::new(
                        ref_at(index),
                        RefusalCause::NoHoleDepth,
                    ));
                }
            },
            Floor::StockTop { .. }
            | Floor::StockBottom { .. }
            | Floor::ModelTop { .. }
            | Floor::ModelBottom { .. }
            | Floor::EachFace { .. }
            | Floor::Z { .. }
            | Floor::Face { .. } => inputs.bottom_z,
            // S11: no floor. The heights refuse it first (`floor_refusal`),
            // and `selection_status` does not call this rule for it. A floor
            // that is not a number refuses below, so no reader cuts it.
            Floor::Unset => f64::NAN,
        };
        if floor_z.is_nan() || floor_z >= inputs.top_z {
            return Err(SelectionRefusal::new(
                ref_at(index),
                RefusalCause::FloorAboveTop {
                    floor_z,
                    top_z: inputs.top_z,
                },
            ));
        }
        spans.push(HoleSpan { base_z, floor_z });
    }
    Ok(spans)
}

/// R-B: the hole planes of `holes` (the fitted holes, in the setup
/// frame): the lowest rim Z of the holes with a rim, and the deepest hole
/// floor with no extra depth and no stock clamp
/// (`rim - HoleDepth::depth_mm`). The heights rule reads them
/// (`session::heights::FacePlanes`).
#[must_use]
pub fn hole_planes(holes: &crate::selection::Holes) -> (Option<f64>, Option<f64>) {
    let rims = holes.as_slice().iter().filter_map(|h| h.rim_z);
    let floors = holes
        .as_slice()
        .iter()
        .filter_map(|h| h.rim_z.zip(h.depth).map(|(rim, d)| rim - d.depth_mm()));
    (rims.reduce(f64::min), floors.reduce(f64::min))
}

/// The deepest floor of `spans`, or `None` for no hole.
#[must_use]
pub fn deepest_hole_floor(spans: &[HoleSpan]) -> Option<f64> {
    spans.iter().map(|s| s.floor_z).reduce(f64::min)
}

/// Why a floor does not resolve. The session puts the toolpath name in
/// front of the text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloorRefusal {
    /// The depth or the Z is not a finite number.
    NotFinite,
    /// [`Floor::ModelTop`] or [`Floor::ModelBottom`] on a toolpath whose
    /// model has no mesh (L-4, D1): a drawing.
    NoModelMesh,
    /// [`Floor::EachFace`] on a selection with no up face (P3.5). The
    /// prologue refuses the area by name (`RefusalCause::NoFaceForFloor`).
    NoFace,
    /// [`Floor::Face`] whose key resolves to no plane (UX-8): no item of
    /// the model matches it, two items match it, or the face points down
    /// in the setup. Generation refuses with the typed selection refusal
    /// of the key (`RefusalCause::FloorFaceLost` and the others).
    FaceLost,
    /// [`Floor::Face`] whose key names an item that is not an area with a
    /// plane Z (UX-8): a path, a point, a circle or a drawing loop.
    NotAFloorItem {
        /// The kind of the item.
        kind: ItemKind,
    },
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
    /// R-B: [`Floor::EachHole`] on a selection with no hole of known
    /// depth. The prologue refuses the hole by name
    /// (`RefusalCause::NoHoleDepth`).
    NoHoleDepth,
    /// R-B: the op cannot cut to this reference ([`Floor::admits`]): each
    /// hole floor on an op with no holes, a face floor on a Drill.
    NotForThisOp {
        /// The `from` token of the reference.
        from: &'static str,
    },
    /// S11 (operator ruling 2026-10-23): no floor is set
    /// ([`Floor::Unset`]). No rule gives a floor for a drawing, so the
    /// operator sets one.
    Unset,
    /// The floor is at or above the resolved top: the ladder would be empty
    /// (ruling L-16).
    AboveTop {
        /// The resolved floor Z.
        floor_z: f64,
        /// The resolved Top Z.
        top_z: f64,
    },
}

/// The refusal text of a floor that is not set (S11). It names the field
/// and the fix; the GUI, MCP and the CLI show this one text.
pub const FLOOR_UNSET_TEXT: &str = "the floor is not set: no rule gives a depth for this \
     op. Set the floor (field `floor`: for example { from = \"top\", depth = 6.0 }, the MCP \
     tool set_toolpath_floor, or the Bottom height row of the Heights tab)";

/// The operator text of a floor that is not set, for the diagnostic detail
/// (S11). [`FLOOR_UNSET_TEXT`] is the generation refusal and keeps the
/// field key and the MCP tool for an agent.
pub const FLOOR_UNSET_DETAIL: &str = "no rule gives a floor depth for this operation. Set \
     the floor in the Bottom height row of the Heights tab.";

impl std::fmt::Display for FloorRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFinite => write!(f, "the floor is not a finite number. Set a floor value"),
            Self::Unset => write!(f, "{FLOOR_UNSET_TEXT}"),
            Self::NoModelMesh => write!(
                f,
                "the floor reads the model (Model top or Model bottom), but the model of the \
                 toolpath has no mesh, so it has no Z extent. Set the Bottom height from the Top \
                 height, the stock or a Z value"
            ),
            Self::NoFace => write!(
                f,
                "the floor is \"each picked face\", but the selection holds no up face. Pick \
                 faces, or set the Bottom height from the Top height, the stock, the model or a \
                 Z value"
            ),
            Self::FaceLost => write!(
                f,
                "the floor is \"a picked face\", but that face is not on the model now (or it \
                 points down in this setup). Use another face as the floor, or set the floor \
                 from another reference"
            ),
            Self::NotAFloorItem { kind } => write!(
                f,
                "the floor \"a picked face\" names a {}; only a face or a model ring has a \
                 plane Z. Use a face as the floor",
                crate::selection::kind_noun(*kind, 1)
            ),
            Self::OutOfDomain { from, name, value } => write!(
                f,
                "the floor {{ from = \"{from}\", {name} = {value} }} is at or above the top, so \
                 the op has nothing to cut. Set a {name} more than 0"
            ),
            Self::NoHoleDepth => write!(
                f,
                "the floor is \"each hole floor\", but no hole of the selection has a known \
                 depth. Pick the rim circles of the holes of a STEP model, or set the Bottom \
                 height from the Top height, the stock, the model or a Z value"
            ),
            Self::NotForThisOp { from } => write!(
                f,
                "the floor {{ from = \"{from}\" }} does not apply to this op. Each hole floor \
                 is for a Drill or a Bore; a Drill takes no face floor. Set the Bottom height \
                 from the Top height, the stock, the model or a Z value"
            ),
            Self::AboveTop { floor_z, top_z } => write!(
                f,
                "the floor Z {floor_z:.3} is at or above the Top height {top_z:.3}, so the op \
                 has nothing to cut. Set the Bottom height below the Top height"
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
            "mm below the resolved Top height of the op (auto: the stock top, or the top of \
             the picked faces; else the pinned top). The default"
        }
        Floor::StockTop { .. } => "mm below the stock top; a negative value stands above it",
        Floor::StockBottom { .. } => {
            "mm below the stock bottom; a positive value cuts into the spoilboard, a negative \
             value leaves a skin"
        }
        Floor::ModelTop { .. } => {
            "mm below the model top; a model with no mesh has no top, so generation refuses it"
        }
        Floor::ModelBottom { .. } => {
            "mm below the model bottom (the lowest Z of the model in the setup); a negative \
             value stands above it. A model with no mesh (a drawing) has no bottom, so \
             generation refuses it. No rule stops it at the stock bottom"
        }
        Floor::EachFace { .. } => {
            "mm below the plane Z of each picked face: each face region is cut to its own \
             floor, from the stock top (or the pinned top). Every closed area of the selection \
             must be a face"
        }
        Floor::Z { .. } => "an absolute Z in the setup frame (frame-dependent)",
        Floor::Face { .. } => {
            "mm below the plane Z of ONE face (`key`: the match key of an up face or a model \
             ring, from list_model_items). The face leaves the picks of the op. A key that \
             matches no item refuses generation"
        }
        Floor::EachHole { .. } => {
            "mm below the floor of each hole of a STEP model: a blind hole to its floor \
             circle, a through hole to the model bottom (never below the stock bottom). Each \
             hole is cut from its own rim. A Drill or a Bore"
        }
        Floor::Unset => "not set: generation refuses until a floor is set (S11)",
    }
}

/// The `floor` entry of `get_operation_schema` for an op whose default
/// floor is `default` (P3.3). It lists each reference with the name and the
/// domain of its number ([`Floor::domain`]), and names the tool that writes
/// the floor. The JSON of a floor is the file JSON.
#[must_use]
pub fn floor_schema_json(default: &Floor, item_use: ItemUse) -> serde_json::Value {
    let variants: Vec<serde_json::Value> = Floor::EACH_REFERENCE
        .iter()
        .filter(|floor| floor.admits(item_use))
        .map(|floor| {
            serde_json::json!({
                "from": floor.from_token(),
                "label": floor.reference_label(),
                "field": floor.value_name(),
                "key": matches!(floor, Floor::Face { .. }),
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
        // S11: a new op has no floor; the schema says `null`.
        "default": (!default.is_unset()).then_some(default),
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
                "floor = { from = \"model_bottom\", depth = -0.5 }",
                Floor::ModelBottom { depth: -0.5 },
            ),
            (
                "floor = { from = \"each_face\", depth = 0.0 }",
                Floor::EachFace { depth: 0.0 },
            ),
            ("floor = { from = \"z\", z = -20.0 }", Floor::Z { z: -20.0 }),
        ];
        let face = "floor = { from = \"face\", depth = 0.5, key = { source = \"toolpath_model\", \
                    kind = \"face\", anchor = [1.0, 2.0], measure = 3.0, brep = 4 } }";
        let read: Holder = toml::from_str(face).unwrap();
        assert!(
            matches!(&read.floor, Floor::Face { key, depth } if key.brep == Some(4) && *depth == 0.5),
            "{:?}",
            read.floor
        );
        let again: Holder = toml::from_str(&toml::to_string(&read).unwrap()).unwrap();
        assert_eq!(again.floor, read.floor, "the face floor after a write");
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
        // UX-8: a face key must name an area with a plane Z.
        let mut key = Floor::EACH_REFERENCE
            .iter()
            .find_map(Floor::face_key)
            .unwrap()
            .clone();
        assert_eq!(
            Floor::Face {
                key: key.clone(),
                depth: 0.0
            }
            .door_refusal(),
            None
        );
        key.kind = ItemKind::Loop;
        assert_eq!(
            Floor::Face {
                key: key.clone(),
                depth: 0.0
            }
            .door_refusal(),
            Some(FloorRefusal::NotAFloorItem {
                kind: ItemKind::Loop
            }),
            "a drawing loop has no plane Z"
        );
        key.z = Some(2.0);
        assert_eq!(Floor::Face { key, depth: 0.0 }.door_refusal(), None);
        for floor in Floor::EACH_REFERENCE {
            let json = serde_json::to_value(&floor).unwrap();
            assert_eq!(json["from"], serde_json::json!(floor.from_token()));
            assert!(json.get(floor.value_name()).is_some(), "{json}");
        }
    }
}
