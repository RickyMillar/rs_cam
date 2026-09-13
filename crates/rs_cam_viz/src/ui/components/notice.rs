//! `EmptyState` (§4.8), `Banner` (§4.9), `NoticeStack` (§4.11) and
//! `NotMeasured` (§4.12).
//!
//! # NoticeStack is the one that matters
//!
//! Operator request, 2026-09-13: *"for cautions/warnings/areas for
//! potentially many stacked notifications. please make sure we have a way to
//! handle n number of x"*.
//!
//! It is **one bounded renderer for N of anything**, and it has six
//! consumers: the toast stack, the load-warnings window, triage advisories,
//! simulation hotspots, the per-toolpath finding set, and inspector cautions.
//! Two of those iterate an unbounded collection today — `app.rs:932` walks
//! every active notification and `app.rs:911` every load warning — so a
//! project with 300 warnings draws 300 rows and pushes everything else off
//! the screen.
//!
//! ## The four rules, and why each exists
//!
//! 1. **Severity outranks the cap.** This is a SAFETY rule, not a layout
//!    rule, and it is the one that must not be got wrong. With a cap of 4 and
//!    a population of 3 `DANGER` plus 40 `CAUTION`, **all three dangers
//!    render.** A collision must never be hidden because forty stale-result
//!    notices got there first.
//! 2. **The overflow row states the TRUE TOTAL** (ruling R1):
//!    `Showing 4 of 293 · Show all`. Never `+289 more`. A remainder makes the
//!    reader do arithmetic to discover the scale of what is hidden, and the
//!    whole defect this component exists to fix is a hidden scale.
//! 3. **Identical items collapse** to one row with a `×N` multiplier, and the
//!    collapse happens BEFORE the cap, so 40 copies of one warning cost one
//!    slot rather than forty.
//! 4. **Order is by severity, then by arrival.** `DANGER`, `CAUTION`,
//!    `UNKNOWN`, `INFO`, `OK` (ruling R10 — §4.11 omitted `OK`).

use std::collections::BTreeMap;

use crate::ui::{
    components::{chip::Role, text},
    tokens,
};

/// One notice: a role, a line of text, and an optional detail.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Notice {
    pub role: Role,
    pub text: String,
    /// Optional grouping key. Notices sharing a group draw under one header.
    pub group: Option<String>,
}

impl Notice {
    pub fn new(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            text: text.into(),
            group: None,
        }
    }

    #[must_use]
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }
}

/// One row after collapsing: a notice and how many identical ones it stands
/// for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CollapsedNotice {
    pub notice: Notice,
    /// How many identical notices this row represents. Always at least 1.
    pub count: usize,
}

/// What [`NoticeStack::resolve`] decided to draw.
///
/// Separated from the drawing so the rules can be tested without a `Context`,
/// and so the sentry asserts the DECISION rather than a pixel.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Resolved {
    /// The rows to draw, already ordered, collapsed and capped.
    pub rows: Vec<CollapsedNotice>,
    /// The true total number of notices, before collapsing or capping.
    ///
    /// This is the number the overflow row states. It is NOT the number of
    /// collapsed rows, and it is NOT a remainder.
    pub total: usize,
    /// True when rows were withheld and an overflow row is needed.
    pub truncated: bool,
}

impl Resolved {
    /// The overflow row's exact words (ruling R1).
    ///
    /// `Showing 4 of 293 · Show all`. Never a remainder.
    #[must_use]
    pub fn overflow_text(&self) -> Option<String> {
        if !self.truncated {
            return None;
        }
        Some(format!(
            "Showing {} of {} \u{00B7} Show all",
            self.rows.len(),
            self.total
        ))
    }
}

/// A bounded renderer for N notices of any kind.
pub struct NoticeStack {
    notices: Vec<Notice>,
    visible_cap: usize,
    expanded: bool,
}

impl NoticeStack {
    /// The default cap. Four rows is what fits beside a panel without
    /// displacing the thing the operator is actually looking at.
    pub const DEFAULT_CAP: usize = 4;

    pub fn new(notices: Vec<Notice>) -> Self {
        Self {
            notices,
            visible_cap: Self::DEFAULT_CAP,
            expanded: false,
        }
    }

    #[must_use]
    pub fn cap(mut self, cap: usize) -> Self {
        self.visible_cap = cap;
        self
    }

    /// Show every row, ignoring the cap (ruling R12).
    ///
    /// Expansion is INLINE. It opens no second surface, because a new surface
    /// would be a new control and UP2 to UP8 add none.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Apply the four rules. Pure, so the sentry can drive it directly.
    #[must_use]
    pub fn resolve(&self) -> Resolved {
        let total = self.notices.len();

        // Rule 3: collapse identical notices BEFORE the cap, so forty copies
        // of one warning cost one slot rather than forty. `BTreeMap` keyed by
        // arrival index of the first occurrence keeps the order stable.
        let mut first_seen: BTreeMap<(Role, String, Option<String>), usize> = BTreeMap::new();
        let mut collapsed: Vec<CollapsedNotice> = Vec::new();
        for notice in &self.notices {
            let key = (notice.role, notice.text.clone(), notice.group.clone());
            match first_seen.get(&key) {
                Some(&idx) => {
                    if let Some(row) = collapsed.get_mut(idx) {
                        row.count += 1;
                    }
                }
                None => {
                    first_seen.insert(key, collapsed.len());
                    collapsed.push(CollapsedNotice {
                        notice: notice.clone(),
                        count: 1,
                    });
                }
            }
        }

        // Rule 4: severity, then arrival. `sort_by_key` is stable, so equal
        // severities keep the order they arrived in.
        collapsed.sort_by_key(|row| row.notice.role.severity_rank());

        // Rule 1: severity outranks the cap. Every DANGER renders, however
        // many there are. This is the safety arm: a collision must never be
        // pushed out by forty stale-result notices.
        let dangers = collapsed
            .iter()
            .filter(|r| r.notice.role == Role::Danger)
            .count();
        let effective_cap = if self.expanded {
            collapsed.len()
        } else {
            self.visible_cap.max(dangers)
        };

        let truncated = collapsed.len() > effective_cap;
        collapsed.truncate(effective_cap);

        Resolved {
            rows: collapsed,
            total,
            truncated,
        }
    }

    /// Draw the stack. Returns true when the operator asked to expand it.
    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let resolved = self.resolve();
        let mut expand_requested = false;

        if resolved.rows.is_empty() {
            return false;
        }

        let mut current_group: Option<&str> = None;
        for row in &resolved.rows {
            // A group header, when the group changes (§4.11).
            if let Some(group) = row.notice.group.as_deref()
                && current_group != Some(group)
            {
                ui.label(text::subhead(group));
                current_group = Some(group);
            }
            notice_row(ui, row);
        }

        if let Some(overflow) = resolved.overflow_text() {
            let response =
                ui.add(egui::Label::new(text::caption(&overflow)).sense(egui::Sense::click()));
            if response.clicked() {
                expand_requested = true;
            }
        }

        expand_requested
    }
}

/// One notice row. Geometry is ruling R13.
fn notice_row(ui: &mut egui::Ui, row: &CollapsedNotice) {
    let role = row.notice.role;
    egui::Frame::default()
        .fill(role.tint())
        .corner_radius(tokens::RADIUS_SM)
        .inner_margin(egui::Margin::symmetric(
            tokens::SPACE_3 as i8,
            tokens::SPACE_2 as i8,
        ))
        .show(ui, |ui| {
            ui.set_min_height(tokens::ROW_DENSE);
            ui.horizontal_wrapped(|ui| {
                if let Some(glyph) = role.glyph() {
                    ui.label(egui::RichText::new(glyph).color(role.text()));
                }
                ui.label(egui::RichText::new(&row.notice.text).color(tokens::TEXT_BODY));
                if row.count > 1 {
                    // Rule 3's multiplier. The reader must be able to see
                    // that one row stands for many.
                    ui.label(text::caption(format!("\u{00D7}{}", row.count)));
                }
            });
        });
}

/// A full-width tinted strip carrying one sentence and at most one action.
pub struct Banner {
    role: Role,
    message: String,
    action: Option<String>,
}

impl Banner {
    pub fn new(role: Role, message: impl Into<String>) -> Self {
        Self {
            role,
            message: message.into(),
            action: None,
        }
    }

    #[must_use]
    pub fn action(mut self, label: impl Into<String>) -> Self {
        self.action = Some(label.into());
        self
    }

    /// Draw the banner. Returns true when its action was clicked.
    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let mut clicked = false;
        egui::Frame::default()
            .fill(self.role.tint())
            .stroke(egui::Stroke::new(1.0, tokens::BORDER))
            .corner_radius(tokens::RADIUS_SM)
            .inner_margin(tokens::SPACE_3)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if let Some(glyph) = self.role.glyph() {
                        ui.label(egui::RichText::new(glyph).color(self.role.text()));
                    }
                    ui.label(text::body_strong(&self.message));
                    if let Some(label) = &self.action {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            clicked = ui
                                .add(super::button::Button::quiet(label.clone()))
                                .clicked();
                        });
                    }
                });
            });
        clicked
    }
}

/// What a surface draws when it has nothing to show.
///
/// Generalises the one good empty state the product already has, at
/// `ui/sim_op_list.rs:129-181`.
pub struct EmptyState {
    headline: String,
    detail: Option<String>,
    action: Option<String>,
}

impl EmptyState {
    pub fn new(headline: impl Into<String>) -> Self {
        Self {
            headline: headline.into(),
            detail: None,
            action: None,
        }
    }

    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// At most ONE action. §4.8 rules that an empty state offering a choice
    /// is not an empty state, it is a menu.
    #[must_use]
    pub fn action(mut self, label: impl Into<String>) -> Self {
        self.action = Some(label.into());
        self
    }

    /// How many buttons this will draw. Never more than one.
    #[must_use]
    pub fn button_count(&self) -> usize {
        usize::from(self.action.is_some())
    }

    /// Draw it. Returns true when the action was clicked.
    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let mut clicked = false;
        ui.vertical_centered(|ui| {
            ui.add_space(tokens::SPACE_6);
            ui.label(text::body_strong(&self.headline));
            if let Some(detail) = &self.detail {
                ui.add_space(tokens::SPACE_2);
                ui.label(text::caption(detail));
            }
            if let Some(label) = &self.action {
                ui.add_space(tokens::SPACE_4);
                clicked = ui
                    .add(super::button::Button::primary(label.clone()))
                    .clicked();
            }
            ui.add_space(tokens::SPACE_6);
        });
        clicked
    }
}

/// The abstention mark: an em dash in `UNKNOWN` (§4.12).
///
/// §2.6: a value the product did not measure "is the em dash `—` in
/// `UNKNOWN`. It is never `0`, never `0.0` and never blank." A zero is a
/// measurement and an abstention is not, and the product has shipped the two
/// looking alike.
pub struct NotMeasured {
    reason: Option<String>,
}

impl Default for NotMeasured {
    fn default() -> Self {
        Self::new()
    }
}

impl NotMeasured {
    #[must_use]
    pub fn new() -> Self {
        Self { reason: None }
    }

    /// Why the measurement was not taken. Shown on hover.
    #[must_use]
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl egui::Widget for NotMeasured {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let response = ui.label(
            egui::RichText::new(tokens::GLYPH_UNKNOWN)
                .font(tokens::font_numeric())
                .color(tokens::UNKNOWN),
        );
        match self.reason {
            Some(r) => response.on_hover_text(r),
            None => response.on_hover_text("not measured"),
        }
    }
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

    fn many(role: Role, n: usize, prefix: &str) -> Vec<Notice> {
        (0..n)
            .map(|i| Notice::new(role, format!("{prefix} {i}")))
            .collect()
    }

    #[test]
    fn severity_outranks_the_cap() {
        // The arm that matters: 3 DANGER + 40 CAUTION, cap 4.
        let mut notices = many(Role::Danger, 3, "collision");
        notices.extend(many(Role::Caution, 40, "stale"));
        let resolved = NoticeStack::new(notices).cap(4).resolve();

        let dangers = resolved
            .rows
            .iter()
            .filter(|r| r.notice.role == Role::Danger)
            .count();
        assert_eq!(
            dangers, 3,
            "every DANGER must render however small the cap. A collision must \
             never be hidden because forty stale notices got there first."
        );
        assert_eq!(resolved.rows.len(), 4, "the cap still bounds the rest");
        assert_eq!(resolved.total, 43, "the TRUE total, not the drawn count");
        assert!(resolved.truncated);
    }

    #[test]
    fn the_overflow_row_states_the_true_total_not_a_remainder() {
        let notices = many(Role::Caution, 293, "warning");
        let resolved = NoticeStack::new(notices).cap(4).resolve();
        let text = resolved.overflow_text().unwrap();
        assert!(
            text.contains("of 293"),
            "the row must state the true total, got {text:?}"
        );
        assert!(
            !text.contains("289"),
            "a remainder makes the reader do arithmetic to find the scale of \
             what is hidden, which is the defect this component exists to \
             fix. Got {text:?}"
        );
        assert_eq!(text, "Showing 4 of 293 \u{00B7} Show all");
    }

    #[test]
    fn identical_notices_collapse_before_the_cap() {
        let notices = vec![Notice::new(Role::Caution, "same"); 40];
        let resolved = NoticeStack::new(notices).cap(4).resolve();
        assert_eq!(resolved.rows.len(), 1, "40 identical notices cost one slot");
        assert_eq!(resolved.rows[0].count, 40, "the multiplier says how many");
        assert_eq!(resolved.total, 40);
        assert!(
            !resolved.truncated,
            "one row holds them all, so nothing was withheld"
        );
    }

    #[test]
    fn order_is_severity_then_arrival() {
        let notices = vec![
            Notice::new(Role::Ok, "passed"),
            Notice::new(Role::Info, "note"),
            Notice::new(Role::Unknown, "not measured"),
            Notice::new(Role::Caution, "stale"),
            Notice::new(Role::Danger, "collision"),
        ];
        let resolved = NoticeStack::new(notices).cap(10).resolve();
        let roles: Vec<Role> = resolved.rows.iter().map(|r| r.notice.role).collect();
        assert_eq!(
            roles,
            vec![
                Role::Danger,
                Role::Caution,
                Role::Unknown,
                Role::Info,
                Role::Ok
            ],
            "ruling R10: five roles, and OK sorts last"
        );
    }

    #[test]
    fn expanding_lifts_the_cap_and_keeps_the_order() {
        let mut notices = many(Role::Caution, 10, "stale");
        notices.push(Notice::new(Role::Danger, "collision"));
        let resolved = NoticeStack::new(notices).cap(2).expanded(true).resolve();
        assert_eq!(resolved.rows.len(), 11, "expansion shows everything");
        assert!(!resolved.truncated, "nothing is withheld when expanded");
        assert_eq!(resolved.rows[0].notice.role, Role::Danger);
    }

    #[test]
    fn an_empty_population_draws_nothing() {
        let resolved = NoticeStack::new(Vec::new()).resolve();
        assert!(resolved.rows.is_empty());
        assert_eq!(resolved.total, 0);
        assert!(!resolved.truncated);
        assert_eq!(resolved.overflow_text(), None);
    }

    #[test]
    fn an_empty_state_never_offers_more_than_one_action() {
        assert_eq!(EmptyState::new("Nothing here").button_count(), 0);
        assert_eq!(
            EmptyState::new("Nothing here")
                .action("Add one")
                .button_count(),
            1
        );
    }
}
