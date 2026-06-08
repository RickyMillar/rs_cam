//! Shared UI component layer (IA cleanup, Wave 1 keystone).
//!
//! The audit's root cause was the same widget reimplemented per surface — the
//! provenance pill 3× (three colours for one signal), the labelled input as
//! `dv`/`dv_pill`, the suggest action hand-rolled in several places, 105+
//! inline section headers, the compare/power/MRR rows trapped private inside
//! `feeds_modal.rs`. This module is the single home for each concept, so a
//! concept has one implementation, therefore one behaviour and one look.
//!
//! Convention (matches the rest of the crate): leaf widgets impl
//! [`egui::Widget`] so callers use `ui.add(Thing::new(..))`; layout helpers are
//! [`UiExt`] methods; anything that mutates takes `events: &mut Vec<AppEvent>`
//! and pushes existing variants — no component owns state or talks to the
//! controller directly.
//!
//! See `planning/ui_audit/ARCHITECTURE.md` for the full contract map.

pub mod compare;
pub mod freshness;
pub mod pill;
pub mod precedence;
pub mod provenance;
pub mod section;
pub mod suggest;
pub mod value_row;

pub use compare::{CompareRow, delta_tag, format_optional, mrr_row, power_bar};
pub use freshness::{Freshness, FreshnessGate};
pub use pill::{CountPill, PillFamily, PillRole};
pub use precedence::PrecedenceField;
pub use provenance::{ProvKind, ProvenanceBadge};
pub use section::{SummaryCard, UiExt};
pub use suggest::{SuggestButton, SuggestScope, Suggestion};
pub use value_row::{ValueRow, ValueRowOutcome};
