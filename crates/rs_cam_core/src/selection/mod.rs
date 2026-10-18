//! Geometry selection: the items of a model (P2.1).
//!
//! An item is a loop, an open path, a point, a circle or a flat face. The
//! import doors fill `LoadedModel::items`; the stock source runs per setup.
//! The selection rules and the resolver come in P2.2.

pub mod item;
pub mod key;
pub mod stock;

pub use item::{FaceItem, Item, ItemError, ItemKind, ItemSet, Shape, SourceRef};
pub use key::{KEY_MATCH_EPS_MM, MatchKey};
pub use stock::stock_items;
