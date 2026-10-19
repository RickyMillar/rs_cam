//! Geometry selection: the items of a model and the selection of an op.
//!
//! An item is a loop, an open path, a point, a circle or a flat face. The
//! import doors fill `LoadedModel::items`; the stock source runs per setup
//! (P2.1); the flat regions of a mesh are faces too (P4.7,
//! `flat_regions`). A [`Selection`] holds rules and picks; [`resolve`]
//! matches it against the items, [`ResolvedSelection::to_setup_frame`]
//! moves the items into the setup frame, and [`view`] gives the items an
//! op cuts or a [`SelectionRefusal`] (P2.2).

pub mod drawing_edit;
pub mod flat_regions;
pub mod item;
pub mod join;
pub mod key;
pub mod near;
pub mod refusal;
pub mod resolve;
pub mod rule;
pub mod stock;
pub mod view;

#[cfg(test)]
mod tests;

pub use drawing_edit::{
    DrawingEdit, DrawingLedger, EditEffect, EditLost, EditOutcome, OpenShape, apply_drawing_edits,
    open_shapes,
};
pub use flat_regions::{FLAT_REGION_Z_BAND_MM, flat_region_items};
pub use item::{FaceItem, Item, ItemCounts, ItemError, ItemKind, ItemSet, Shape, SourceRef};
pub use key::{KEY_MATCH_EPS_MM, MatchKey};
pub use near::{PickLine, items_near, polyline_distance};
pub use refusal::{
    ItemRef, RefusalCause, SelectionNote, SelectionRefusal, kind_not_cut_reason, kind_noun,
};
pub use resolve::{
    ExcludedItem, LostPick, MatchedBy, MeshFaces, ResolveScope, ResolvedItem, ResolvedSelection,
    face_in_setup_frame, resolve,
};
pub use rule::{
    DiameterRange, FaceFilter, FacesByFacing, Facing, Filter, KindFilter, Rule, Selection,
    SelectionEdit, ZRange,
};
pub use stock::stock_items;
pub use view::{Curves, Hole, Holes, ItemRole, OpItems, Regions, view, view_boundary};
