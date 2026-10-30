//! Shared UI component layer (IA cleanup, Wave 1 keystone).
//!
//! The audit's root cause was the same widget reimplemented per surface — the
//! provenance pill 3× (three colours for one signal), the labelled input as
//! `dv`/`dv_pill`, the suggest action hand-rolled in several places, the
//! inline section headers, the compare/power/MRR rows trapped private inside
//! `feeds_modal.rs`. This module is the single home for each concept, so a
//! concept has one implementation, therefore one behaviour and one look.
//!
//! This doc used to claim "105+ inline section headers". UI-03 closed that
//! count: every standalone section title now calls [`SectionHeader`] or
//! `UiExt::named_section`. The `.small().strong()` chains that remain are
//! grid column headers, notices and active-row emphasis — not headers —
//! and `component_contracts_up2` names each one with its reason.
//!
//! Convention (matches the rest of the crate): leaf widgets impl
//! [`egui::Widget`] so callers use `ui.add(Thing::new(..))`; layout helpers are
//! [`UiExt`] methods; anything that mutates takes `events: &mut Vec<AppEvent>`
//! and pushes existing variants — no component owns state or talks to the
//! controller directly.
//!
//! See `planning/ui_audit/ARCHITECTURE.md` for the full contract map.

pub mod button;
pub mod card;
pub mod chip;
pub mod choice_row;
pub mod compare;
pub mod data_table;
pub mod focus_ring;
pub mod format;
pub mod freshness;
pub mod histogram;
pub mod kv_row;
pub mod line;
pub mod list_box;
pub mod motion;
pub mod notes_counter;
pub mod notice;
pub mod number;
pub mod pill;
pub mod precedence;
pub mod prop_row;
pub mod provenance;
pub mod section;
pub mod sparkline;
pub mod status_line;
pub mod suggest;
pub mod tab_strip;
pub mod text;
pub mod value_row;

pub use button::{Button, Variant as ButtonVariant};
pub use card::{Card, SectionHeader, scrim};
pub use chip::{Role, StatusChip};
pub use choice_row::ChoiceRow;
pub use compare::{CompareRow, delta_tag, format_optional, mrr_row, power_bar};
pub use data_table::{DataRow, DataTable, DataTableResponse, Details, NOT_MEASURED_TEXT, Source};
pub use freshness::{Freshness, FreshnessGate};
pub use histogram::{DistributionChart, DistributionChartResponse};
pub use kv_row::{KeyValueRow, Value as KeyValue};
pub use line::OneLine;
pub use list_box::{Act, ListBox, ListLine, Mark, list_header, list_line, list_placeholder, list_row};
pub use notes_counter::NotesCounter;
pub use notice::{
    Banner, CollapsedNotice, EmptyState, IssueStackResponse, NotMeasured, Notice, NoticeStack,
    Resolved,
};
pub use number::{NumberSpec, number_field, optional_number_field};
pub use pill::{CountPill, PillFamily, PillRole};
pub use precedence::PrecedenceField;
pub use prop_row::{PropRow, PropRowResponse, RowRects};
pub use provenance::{ProvKind, ProvenanceBadge};
pub use section::{SummaryCard, UiExt};
pub use sparkline::{ChartFace, ChartFlip, Sparkline, SparklineResponse, TraceOpen};
pub use status_line::StatusLine;
pub use suggest::{SuggestButton, SuggestScope, Suggestion};
pub use tab_strip::{TabBadge, TabStrip};
pub use value_row::{OptionalValueRow, ValueRow, ValueRowOutcome};
