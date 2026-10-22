//! Strongly-typed identifiers shared across the CAM data model.
//!
//! R3 (tech-debt review 2026-06-10): toolpath *ids* and *indexes into
//! `session.toolpath_configs`* used to unify as bare `usize`, which made
//! `toolpath_id == index`-shaped comparisons compile silently (the WANAKA
//! "Back Rough is index 1 but id 4" wrong-lookup class). The id newtypes
//! exist so the compiler surfaces every such confusion. Indexes stay
//! `usize` on purpose — do not newtype them.
//!
//! F004 (W13b): every id type lives in this file. The inner value is
//! private, so an index cannot go into an id slot by accident. These
//! doors make an id:
//!
//! - the session allocators and the project loader (`pub(crate) fn mint`),
//! - serde (`#[serde(transparent)]`: the file and the wire hold the bare
//!   integer, byte for byte as before the newtype),
//! - [`ToolpathId::from_wire`] and its twins: the id that a caller names
//!   on a wire (MCP) or that a job runner numbers. A lookup refuses an id
//!   that does not exist, so this door cannot make a false record.
//! - [`ToolpathId::placeholder`] and its twins: the value 0 that a draft
//!   or a fallback holds where no allocated id exists.
//! - `from_raw`, for tests only (`test` or the `test-support` feature).
//!
//! [`raw`](ToolpathId::raw) reads the inner value.

use serde::{Deserialize, Serialize};

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(usize);

        impl $name {
            /// The inner value: the integer that the file and the wire hold.
            #[must_use]
            pub const fn raw(self) -> usize {
                self.0
            }

            /// The allocator door: only session code makes a new id.
            #[must_use]
            pub(crate) const fn mint(raw: usize) -> Self {
                Self(raw)
            }

            /// The value 0 that a draft or a fallback holds where no
            /// allocated id exists. An add door overwrites the id of a
            /// draft. A lookup of a fallback finds the record with id 0,
            /// or nothing.
            #[must_use]
            pub const fn placeholder() -> Self {
                Self(0)
            }

            /// The id that a caller names on a wire (MCP) or that a job
            /// runner numbers. It is a claim, not a proof: the lookup
            /// refuses an id that does not exist.
            #[must_use]
            pub const fn from_wire(raw: usize) -> Self {
                Self(raw)
            }

            /// A test door: an id from a literal.
            #[cfg(any(test, feature = "test-support"))]
            #[must_use]
            pub const fn from_raw(raw: usize) -> Self {
                Self(raw)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

define_id!(
    /// Stable identity of a toolpath (`ToolpathConfig.id`).
    ///
    /// This is *not* an index into `session.toolpath_configs`: configs can be
    /// reordered or deleted, so id and position diverge. Map between them via
    /// `toolpath_configs[index].id` or
    /// `toolpath_configs.iter().position(|tc| tc.id == id)`.
    ///
    /// `#[serde(transparent)]` keeps the wire shape identical to a bare
    /// integer — SimulationCutTrace JSON dumps, project TOML files, and MCP
    /// payloads are byte-compatible with the pre-newtype format.
    ToolpathId
);

define_id!(
    /// Unique identifier for a tool (`ToolConfig.id`).
    ToolId
);

define_id!(
    /// Unique identifier for a loaded model.
    ModelId
);

define_id!(
    /// Unique identifier for a setup (workholding / orientation context).
    SetupId
);

define_id!(
    /// Unique identifier for an obstacle within a setup (K1). An obstacle is
    /// a fixture or a keep-out zone: the two are one concept.
    ObstacleId
);

define_id!(
    /// Unique identifier for a stock change within a setup (S2).
    StockChangeId
);
