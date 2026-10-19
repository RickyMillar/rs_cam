//! Geometry selection: the items of a model and the selection of an op.
//!
//! An item is a loop, an open path, a point, a circle or a flat face. The
//! import doors fill `LoadedModel::items`; the stock source runs per setup
//! (P2.1). A [`Selection`] holds rules and picks; [`resolve`] matches it
//! against the items, [`ResolvedSelection::to_setup_frame`] moves the items
//! into the setup frame, and [`view`] gives the items an op cuts or a
//! [`SelectionRefusal`] (P2.2).

pub mod item;
pub mod key;
pub mod refusal;
pub mod resolve;
pub mod rule;
pub mod stock;
pub mod view;

#[cfg(test)]
mod tests;

pub use item::{FaceItem, Item, ItemCounts, ItemError, ItemKind, ItemSet, Shape, SourceRef};
pub use key::{KEY_MATCH_EPS_MM, MatchKey};
pub use refusal::{ItemRef, RefusalCause, SelectionNote, SelectionRefusal};
pub use resolve::{
    LostPick, MatchedBy, ResolveScope, ResolvedItem, ResolvedSelection, face_in_setup_frame,
    resolve,
};
pub use rule::{
    DiameterRange, FaceFilter, Facing, Filter, KindFilter, Rule, Selection, SelectionEdit, ZRange,
};
pub use stock::stock_items;
pub use view::{Curves, Hole, Holes, OpItems, Regions, view};
