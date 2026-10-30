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
    /// Optional hover text of the row: the fix or the cause, so the row
    /// stays one short line (G-D).
    pub detail: Option<String>,
    /// The label of the one Fix button of an issue row (W1). It is not
    /// part of the collapse key.
    pub fix: Option<String>,
}

impl Notice {
    pub fn new(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            text: text.into(),
            group: None,
            detail: None,
            fix: None,
        }
    }

    /// The label of the one Fix button of an issue row
    /// ([`NoticeStack::show_issues`]).
    #[must_use]
    pub fn fix(mut self, label: impl Into<String>) -> Self {
        self.fix = Some(label.into());
        self
    }

    /// The hover text of the row.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
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
    /// The input index of the first notice this row stands for.
    pub first: usize,
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
        self.truncated
            .then(|| overflow_line(self.rows.len(), self.total))
    }
}

/// The remainder line of a bounded list (ruling R1): `Showing 4 of 293 ·
/// Show all`. Never a remainder. A list that is not a `NoticeStack` (the
/// toolpath footer findings, G-E) takes the same words from here.
#[must_use]
pub fn overflow_line(shown: usize, total: usize) -> String {
    format!("Showing {shown} of {total} \u{00B7} Show all")
}

/// Draw the remainder line. True when it was clicked (the reader asked to
/// see every row).
pub fn overflow_row(ui: &mut egui::Ui, shown: usize, total: usize) -> bool {
    ui.add(egui::Label::new(text::caption(overflow_line(shown, total))).sense(egui::Sense::click()))
        .clicked()
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
    pub(crate) const DEFAULT_CAP: usize = 4;

    /// The cap of an issue stack (W1): at most three issue rows under the
    /// inspector header.
    pub const ISSUE_CAP: usize = 3;

    /// An issue stack (W1): the cap is [`Self::ISSUE_CAP`]; draw it with
    /// [`Self::show_issues`]. The four rules stay: a `Danger` still
    /// outranks the cap.
    pub fn issues(notices: Vec<Notice>) -> Self {
        Self::new(notices).cap(Self::ISSUE_CAP)
    }

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
        for (index, notice) in self.notices.iter().enumerate() {
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
                        first: index,
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

        if resolved.truncated && overflow_row(ui, resolved.rows.len(), resolved.total) {
            expand_requested = true;
        }

        expand_requested
    }
}

/// What [`NoticeStack::show_issues`] returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IssueStackResponse {
    /// The input index of the notice whose Fix button was clicked.
    pub fix: Option<usize>,
    /// The operator clicked the overflow row to see every issue. The
    /// caller then draws the stack with `expanded(true)` (ruling R12).
    pub expand: bool,
}

impl NoticeStack {
    /// Draw the stack as issue rows (W1): one line per row, the severity
    /// glyph, the face text (it truncates; the hover has the full text and
    /// the detail) and at most one Fix button. The overflow row is the R1
    /// line: it states the true total, never a remainder.
    pub fn show_issues(self, ui: &mut egui::Ui) -> IssueStackResponse {
        let resolved = self.resolve();
        let mut out = IssueStackResponse::default();
        for row in &resolved.rows {
            if issue_row(ui, row) {
                out.fix = Some(row.first);
            }
        }
        if resolved.truncated && overflow_row(ui, resolved.rows.len(), resolved.total) {
            out.expand = true;
        }
        out
    }
}

/// The hover text of an issue row: the full face, then the detail. Pure.
#[must_use]
pub fn issue_hover(notice: &Notice) -> String {
    match notice.detail.as_deref().filter(|d| !d.is_empty()) {
        Some(detail) => format!("{}\n\n{detail}", notice.text),
        None => notice.text.clone(),
    }
}

/// One issue row: `ROW_ACTION` high, the role tint, the glyph, the face
/// and the Fix button at the right edge. True when Fix was clicked.
fn issue_row(ui: &mut egui::Ui, row: &CollapsedNotice) -> bool {
    let role = row.notice.role;
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, tokens::ROW_ACTION), egui::Sense::hover());
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::from(tokens::RADIUS_SM),
        role.tint(),
    );
    let inner = rect.shrink2(egui::vec2(tokens::SPACE_3, 0.0));
    let child = |ui: &mut egui::Ui, r: egui::Rect| {
        let mut c = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(r)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        c.set_clip_rect(r.intersect(ui.clip_rect()));
        c
    };
    let text_width = |ui: &egui::Ui, t: &str, font: egui::FontId| {
        ui.painter()
            .layout_no_wrap(t.to_owned(), font, tokens::TEXT_BODY)
            .size()
            .x
    };
    let body = egui::FontId::proportional(tokens::SIZE_BODY);
    // From the right: the Fix button, then the multiplier. The face takes
    // what is left, so the button never moves to a second line.
    let mut right = inner.right();
    let mut fixed = false;
    if let Some(fix) = row.notice.fix.as_deref() {
        let font = ui
            .style()
            .text_styles
            .get(&egui::TextStyle::Button)
            .cloned()
            .unwrap_or_default();
        let w = text_width(ui, fix, font) + 2.0 * ui.spacing().button_padding.x;
        let r = egui::Rect::from_min_max(
            egui::pos2(right - w, rect.top()),
            egui::pos2(right, rect.bottom()),
        );
        fixed = child(ui, r)
            .add(super::button::Button::quiet(fix))
            .clicked();
        right = r.left() - tokens::SPACE_2;
    }
    if row.count > 1 {
        let multiplier = format!("\u{00D7}{}", row.count);
        let w = text_width(
            ui,
            &multiplier,
            egui::FontId::proportional(tokens::SIZE_CAPTION),
        );
        let r = egui::Rect::from_min_max(
            egui::pos2(right - w, rect.top()),
            egui::pos2(right, rect.bottom()),
        );
        let _ = child(ui, r).label(text::caption(multiplier));
        right = r.left() - tokens::SPACE_2;
    }
    let mut left = inner.left();
    if let Some(glyph) = role.glyph() {
        let w = text_width(ui, glyph, body);
        let r = egui::Rect::from_min_max(
            egui::pos2(left, rect.top()),
            egui::pos2(left + w, rect.bottom()),
        );
        let _ = child(ui, r).label(egui::RichText::new(glyph).color(role.text()));
        left = r.right() + tokens::SPACE_3;
    }
    let face = egui::Rect::from_min_max(
        egui::pos2(left, rect.top()),
        egui::pos2(right.max(left), rect.bottom()),
    );
    let _ = child(ui, face)
        .add(
            egui::Label::new(egui::RichText::new(&row.notice.text).color(tokens::TEXT_BODY))
                .truncate(),
        )
        .on_hover_text(issue_hover(&row.notice));
    let _ = response.on_hover_text(issue_hover(&row.notice));
    fixed
}

/// One notice row. Geometry is ruling R13.
fn notice_row(ui: &mut egui::Ui, row: &CollapsedNotice) {
    let role = row.notice.role;
    let frame = egui::Frame::default()
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
    if let Some(detail) = row.notice.detail.as_deref() {
        frame.response.on_hover_text(detail);
    }
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
    glyph: Option<String>,
}

impl EmptyState {
    pub fn new(headline: impl Into<String>) -> Self {
        Self {
            headline: headline.into(),
            detail: None,
            action: None,
            glyph: None,
        }
    }

    /// A 24-point outline glyph above the headline, in `INK_35` (§4.8).
    #[must_use]
    pub fn glyph(mut self, glyph: impl Into<String>) -> Self {
        self.glyph = Some(glyph.into());
        self
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
            // §4.8: centred, and at most 280 points wide. An empty state that
            // runs the full panel width reads as a broken layout.
            ui.set_max_width(tokens::EMPTY_STATE_MAX_WIDTH);
            ui.add_space(tokens::SPACE_6);
            if let Some(glyph) = &self.glyph {
                ui.label(
                    egui::RichText::new(glyph)
                        .size(tokens::EMPTY_STATE_GLYPH_SIZE)
                        .color(tokens::INK_35),
                );
                ui.add_space(tokens::SPACE_2);
            }
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
    /// W6: a caption after the glyph, on the same line.
    text: Option<String>,
}

impl Default for NotMeasured {
    fn default() -> Self {
        Self::new()
    }
}

impl NotMeasured {
    #[must_use]
    pub fn new() -> Self {
        Self {
            reason: None,
            text: None,
        }
    }

    /// W6: a caption after the glyph, on the same line (a `OneLine`), for
    /// a surface that says in words what it did not measure.
    #[must_use]
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Why the measurement was not taken. Shown on hover.
    #[must_use]
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl egui::Widget for NotMeasured {
    fn ui(mut self, ui: &mut egui::Ui) -> egui::Response {
        if let Some(text) = self.text.take() {
            return ui
                .horizontal(|ui| {
                    let glyph = ui.add(self);
                    let _ = ui.add(super::OneLine::new(text));
                    glyph
                })
                .inner;
        }
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
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
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

    /// W1: an issue stack shows at most three rows and states the true
    /// total on the overflow row.
    #[test]
    fn an_issue_stack_shows_at_most_three_w1() {
        let resolved = NoticeStack::issues(many(Role::Caution, 7, "check")).resolve();
        assert_eq!(resolved.rows.len(), 3);
        assert_eq!(
            resolved.overflow_text().unwrap(),
            "Showing 3 of 7 \u{00B7} Show all"
        );
        let few = NoticeStack::issues(many(Role::Caution, 2, "check")).resolve();
        assert_eq!(few.rows.len(), 2);
        assert!(!few.truncated);
    }

    /// W1 keeps rule 1: every blocking issue renders, even past the cap.
    #[test]
    fn every_blocking_issue_renders_past_the_cap_w1() {
        let mut notices = many(Role::Danger, 4, "blocked");
        notices.extend(many(Role::Caution, 3, "check"));
        let resolved = NoticeStack::issues(notices).resolve();
        assert_eq!(resolved.rows.len(), 4);
        assert!(resolved.rows.iter().all(|r| r.notice.role == Role::Danger));
    }

    /// W1: a row keeps the input index of its first notice, so a Fix click
    /// names the notice after the sort.
    #[test]
    fn a_row_names_its_first_notice_w1() {
        let notices = vec![
            Notice::new(Role::Caution, "check").fix("Fix"),
            Notice::new(Role::Danger, "blocked"),
        ];
        let resolved = NoticeStack::issues(notices).resolve();
        let firsts: Vec<usize> = resolved.rows.iter().map(|r| r.first).collect();
        assert_eq!(firsts, vec![1, 0]);
    }

    /// W1: an issue row is one line of `ROW_ACTION`. A long face
    /// truncates inside the row, the Fix button stays on the line, and the
    /// hover has the full face and the detail.
    #[test]
    fn an_issue_row_is_one_truncated_line_w1() {
        const FACE: &str = "Depth per pass 1.5 mm is over the rigid limit of 0.83 mm for this tool";
        let ctx = egui::Context::default();
        tokens::apply(&ctx);
        tokens::apply_fonts(&ctx);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        warmup.textures_delta.clear();
        let mut top = 0.0;
        let mut bottom = 0.0;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_max_width(300.0);
            top = ui.cursor().top();
            let notice = Notice::new(Role::Caution, FACE)
                .detail("The rigid limit comes from the tool stick-out.")
                .fix("Fix");
            let _ = NoticeStack::issues(vec![notice]).show_issues(ui);
            bottom = ui.min_rect().bottom();
        });
        assert!(
            (bottom - top - tokens::ROW_ACTION).abs() < 0.5,
            "{top} {bottom}"
        );
        // A `Button` lays out its label twice (a transparent run that
        // reserves the space, then the painted one); keep the runs that
        // paint, with their elided flag.
        let texts: Vec<(String, egui::Rect, bool)> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::epaint::Shape::Text(t)
                    if t.galley
                        .job
                        .sections
                        .iter()
                        .any(|s| s.format.color != egui::Color32::TRANSPARENT) =>
                {
                    Some((
                        t.galley.job.text.clone(),
                        c.shape.visual_bounding_rect(),
                        t.galley.elided,
                    ))
                }
                _ => None,
            })
            .collect();
        out.textures_delta.clear();
        let face = texts
            .iter()
            .find(|(t, _, _)| t.starts_with("Depth"))
            .unwrap();
        assert!(face.2, "not truncated");
        let fix = texts.iter().find(|(t, _, _)| t == "Fix").unwrap();
        assert!(fix.1.left() >= face.1.right(), "Fix after the face");
        assert!(fix.1.right() <= 300.5, "Fix inside the row");
        assert!(
            (fix.1.center().y - face.1.center().y).abs() < tokens::SPACE_2,
            "one line: {fix:?} {face:?}"
        );
        let hover = issue_hover(&Notice::new(Role::Caution, FACE).detail("Why."));
        assert_eq!(hover, format!("{FACE}\n\nWhy."));
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
