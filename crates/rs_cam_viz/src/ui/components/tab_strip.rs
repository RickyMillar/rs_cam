//! `TabStrip` — the one strip of tabs inside a panel.
//!
//! The toolpath inspector drew its own strip in `properties/tab_badges.rs`.
//! The Simulation inspector needs the same strip (inspector clean-up round
//! 3, operator 2026-10-13: "can we de-clutter by moving the sections into
//! tabs"). This file is now the one renderer for both. The workspace bar is
//! chrome and keeps its own tabs (`workspace_bar.rs`).
//!
//! A tab is a label and an optional badge:
//!
//! - `TabBadge::Dot` — a small dot in a role colour in the top-left corner
//!   of the tab, inside its padding. The dot adds no width, so a dot never
//!   changes the size of the labels (W3 fix: a dot pushed the five R2 tabs
//!   at 340 points into the compact form). In the `»` menu the dot is a
//!   text run before the label.
//! - `TabBadge::Count` — the caution glyph and a count after the label, in
//!   the colour of its role. A count of 0 shows no badge.
//!
//! Compact form (stepGT, operator ruling 2026-10-28): when the tabs at
//! their full size do not fit the width, the strip draws them compact:
//! caption-size labels, no minimum tab width, a 2-point padding and no
//! gap. The six toolpath tabs then fit one line of the 280-point
//! inspector. The labels do not change.
//!
//! The strip never takes a second row (W1, F10: a dot wrapped the
//! toolpath strip to two rows). When the compact tabs do not fit either,
//! the strip shows the head of the tabs and a `»` menu for the tail. The
//! active tab is always on the strip; a hidden tab with a badge gives its
//! badge colour to the `»`. A tab past the panel edge is still a defect
//! (UR1), so the strip measures before it draws.
//!
//! The active tab takes `ACCENT_QUIET` and an `ACCENT` underline;
//! its radius is on the two top corners, because a tab joins the panel
//! below it (`DESIGN_SPEC.md` §2.2).

use crate::ui::components::Role;
use crate::ui::tokens;

/// The smallest width of a tab, in points. Below it a short label such as
/// "Dressup" wrapped to two rows.
const TAB_MIN_WIDTH: f32 = 55.0;

/// The horizontal padding of a compact tab, in points.
const COMPACT_PADDING_X: f32 = tokens::SPACE_1;

/// The glyph before the count of a count badge.
pub const COUNT_GLYPH: &str = "\u{26A0}";

/// The radius of the dot badge, in points.
const DOT_RADIUS: f32 = 2.0;

/// The inset of the dot centre from the top-left corner of the tab, in
/// points. The dot then sits in the padding, above the cap height of a
/// body label in a `ROW_ACTION` tab.
const DOT_INSET: f32 = 4.5;

/// The badge of one tab.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabBadge {
    /// A dot in this colour in the top-left corner of the tab.
    Dot(egui::Color32),
    /// The count glyph and a count after the label, in the colour of the
    /// role. A count of 0 shows no badge.
    Count(usize, Role),
}

/// One tab: the value it selects, its label and its badge.
struct Tab<'a, T> {
    value: T,
    label: &'a str,
    badge: Option<TabBadge>,
}

/// A strip of tabs that writes the selected value.
#[must_use]
pub struct TabStrip<'a, T: Copy + PartialEq> {
    active: &'a mut T,
    tabs: Vec<Tab<'a, T>>,
}

impl<'a, T: Copy + PartialEq> TabStrip<'a, T> {
    /// A strip with no tabs that writes `active`.
    pub fn new(active: &'a mut T) -> Self {
        Self {
            active,
            tabs: Vec::new(),
        }
    }

    /// Add a tab with no badge.
    pub fn tab(self, value: T, label: &'a str) -> Self {
        self.tab_badge(value, label, None)
    }

    /// Add a tab with an optional badge.
    pub fn tab_badge(mut self, value: T, label: &'a str, badge: Option<TabBadge>) -> Self {
        let badge = badge.filter(|badge| !matches!(badge, TabBadge::Count(0, _)));
        self.tabs.push(Tab {
            value,
            label,
            badge,
        });
        self
    }

    /// Draw the strip on one row. Returns true when a click selected
    /// another tab.
    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        let active = self.active;
        let width = ui.available_width();
        let layout = plan(ui, &self.tabs, *active, width);
        let compact = layout.mode != Mode::Full;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if compact {
                ui.spacing_mut().button_padding.x = COMPACT_PADDING_X;
            }
            let size = if compact {
                tokens::SIZE_CAPTION
            } else {
                tokens::SIZE_BODY
            };
            let min_width = if compact { 0.0 } else { TAB_MIN_WIDTH };
            for &index in &layout.visible {
                let Some(tab) = self.tabs.get(index) else {
                    continue;
                };
                let is_active = *active == tab.value;
                let (fill, text_color) = if is_active {
                    (tokens::ACCENT_QUIET, tokens::TEXT_STRONG)
                } else {
                    (egui::Color32::TRANSPARENT, tokens::TEXT_MUTED)
                };
                let button = egui::Button::new(tab_label(ui, tab, text_color, size, false))
                    .fill(fill)
                    .corner_radius(egui::CornerRadius {
                        nw: tokens::RADIUS_SM,
                        ne: tokens::RADIUS_SM,
                        sw: 0,
                        se: 0,
                    })
                    .min_size(egui::vec2(min_width, tokens::ROW_ACTION));
                let response = ui.add(button);
                if let Some(TabBadge::Dot(color)) = tab.badge {
                    let corner = response.rect.left_top() + egui::vec2(DOT_INSET, DOT_INSET);
                    ui.painter().circle_filled(corner, DOT_RADIUS, color);
                }
                if response.clicked() && !is_active {
                    *active = tab.value;
                    changed = true;
                }
                if is_active {
                    let rect = response.rect;
                    ui.painter().line_segment(
                        [rect.left_bottom(), rect.right_bottom()],
                        egui::Stroke::new(2.0_f32, tokens::ACCENT),
                    );
                }
                if !compact {
                    ui.add_space(tokens::SPACE_1);
                }
            }
            if !layout.hidden.is_empty() {
                let hidden: Vec<&Tab<'_, T>> = layout
                    .hidden
                    .iter()
                    .filter_map(|&index| self.tabs.get(index))
                    .collect();
                if let Some(value) = more_menu(ui, &hidden, size) {
                    *active = value;
                    changed = true;
                }
            }
        });
        changed
    }
}

/// How the strip fits its width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Every tab at full size.
    Full,
    /// Every tab compact.
    Compact,
    /// The head of the tabs compact, the tail in the `»` menu.
    Overflow,
}

/// The decision of [`plan`]: the mode and the tab indices on the strip and
/// in the `»` menu, each in strip order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Layout {
    mode: Mode,
    visible: Vec<usize>,
    hidden: Vec<usize>,
}

/// Decide the one-row layout of `tabs` in `width`.
///
/// 1. Full size when it fits.
/// 2. Else compact: no gap, no minimum width, caption labels.
/// 3. Else the longest compact head that fits beside the `»` button. When
///    the active tab is in the tail, the head gets shorter until the active
///    tab also fits after it.
fn plan<T: PartialEq + Copy>(ui: &egui::Ui, tabs: &[Tab<'_, T>], active: T, width: f32) -> Layout {
    let all: Vec<usize> = (0..tabs.len()).collect();
    if full_width(ui, tabs) <= width {
        return Layout {
            mode: Mode::Full,
            visible: all,
            hidden: Vec::new(),
        };
    }
    let compact: Vec<f32> = tabs.iter().map(|tab| compact_width(ui, tab)).collect();
    if compact.iter().sum::<f32>() <= width {
        return Layout {
            mode: Mode::Compact,
            visible: all,
            hidden: Vec::new(),
        };
    }
    let budget = width - more_width(ui);
    let active_index = tabs.iter().position(|tab| tab.value == active);
    let width_of = |index: usize| compact.get(index).copied().unwrap_or(0.0);
    let mut head = 0;
    let mut used = 0.0;
    while head < tabs.len() && used + width_of(head) <= budget {
        used += width_of(head);
        head += 1;
    }
    let mut visible: Vec<usize> = (0..head).collect();
    if let Some(a) = active_index
        && a >= head
    {
        // The active tab is in the tail: drop head tabs until it fits.
        while !visible.is_empty() && used + width_of(a) > budget {
            if let Some(last) = visible.pop() {
                used -= width_of(last);
            }
        }
        visible.push(a);
    }
    let hidden = all.into_iter().filter(|i| !visible.contains(i)).collect();
    Layout {
        mode: Mode::Overflow,
        visible,
        hidden,
    }
}

/// The glyph of the button that opens the menu of the hidden tabs.
const MORE_GLYPH: &str = "\u{00BB}";

/// The colour of a badge, for the dot on the `»` button.
fn badge_color(badge: TabBadge) -> egui::Color32 {
    match badge {
        TabBadge::Dot(color) => color,
        TabBadge::Count(_, role) => role.text(),
    }
}

/// The text of the `»` button. A hidden badge shows as a dot before the
/// glyph, so the menu does not hide a problem.
fn more_text(style: &egui::Style, dot: Option<egui::Color32>, size: f32) -> egui::WidgetText {
    let mut job = egui::text::LayoutJob::default();
    if let Some(color) = dot {
        egui::RichText::new("\u{25CF} ")
            .size(tokens::SIZE_CAPTION)
            .color(color)
            .append_to(
                &mut job,
                style,
                egui::FontSelection::Default,
                egui::Align::Center,
            );
    }
    egui::RichText::new(MORE_GLYPH)
        .size(size)
        .color(tokens::TEXT_MUTED)
        .strong()
        .append_to(
            &mut job,
            style,
            egui::FontSelection::Default,
            egui::Align::Center,
        );
    egui::WidgetText::LayoutJob(std::sync::Arc::new(job))
}

/// The width of the `»` button, with room for a dot, at compact size.
fn more_width(ui: &egui::Ui) -> f32 {
    let galley = more_text(ui.style(), Some(tokens::TEXT_MUTED), tokens::SIZE_CAPTION).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    galley.size().x + 2.0 * COMPACT_PADDING_X
}

/// Draw the `»` button and its menu of the hidden tabs. Returns the value
/// of the tab the operator picked in the menu.
fn more_menu<T: Copy>(ui: &mut egui::Ui, hidden: &[&Tab<'_, T>], size: f32) -> Option<T> {
    let dot = hidden.iter().find_map(|tab| tab.badge.map(badge_color));
    let button = egui::Button::new(more_text(ui.style(), dot, size))
        .fill(egui::Color32::TRANSPARENT)
        .min_size(egui::vec2(0.0, tokens::ROW_ACTION));
    let response = ui.add(button).on_hover_text(
        hidden
            .iter()
            .map(|tab| tab.label)
            .collect::<Vec<_>>()
            .join(", "),
    );
    let mut picked = None;
    let _ = egui::Popup::menu(&response).show(|ui| {
        for tab in hidden {
            let label = tab_label(ui, tab, tokens::TEXT_BODY, tokens::SIZE_BODY, true);
            if ui.add(egui::Button::new(label).frame(false)).clicked() {
                picked = Some(tab.value);
            }
        }
    });
    picked
}

/// The width of one compact tab: its caption-size label and the compact
/// padding, with no minimum width and no gap.
fn compact_width<T>(ui: &egui::Ui, tab: &Tab<'_, T>) -> f32 {
    let galley = tab_label(ui, tab, tokens::TEXT_STRONG, tokens::SIZE_CAPTION, false).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    galley.size().x + 2.0 * COMPACT_PADDING_X
}

/// The width of the strip at full size on one line: each tab's label and
/// padding, at least [`TAB_MIN_WIDTH`], and the gap after it.
fn full_width<T>(ui: &egui::Ui, tabs: &[Tab<'_, T>]) -> f32 {
    let padding = 2.0 * ui.spacing().button_padding.x;
    tabs.iter()
        .map(|tab| {
            let galley = tab_label(ui, tab, tokens::TEXT_STRONG, tokens::SIZE_BODY, false)
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Extend),
                    f32::INFINITY,
                    egui::TextStyle::Button,
                );
            (galley.size().x + padding).max(TAB_MIN_WIDTH) + tokens::SPACE_1
        })
        .sum()
}

/// The text of a count badge: the glyph and the count.
#[must_use]
pub fn count_text(count: usize) -> String {
    format!("{COUNT_GLYPH} {count}")
}

/// The label of one tab, with its count badge, as one text run. A dot
/// badge is a text run only when `dot_text` is true (the `»` menu); on the
/// strip the painter draws it in the corner, with no width.
fn tab_label<T>(
    ui: &egui::Ui,
    tab: &Tab<'_, T>,
    text_color: egui::Color32,
    size: f32,
    dot_text: bool,
) -> egui::WidgetText {
    let label = || {
        egui::RichText::new(tab.label)
            .size(size)
            .color(text_color)
            .strong()
    };
    let mut job = egui::text::LayoutJob::default();
    let mut append = |text: egui::RichText| {
        text.append_to(
            &mut job,
            ui.style(),
            egui::FontSelection::Default,
            egui::Align::Center,
        );
    };
    match tab.badge {
        None => return label().into(),
        Some(TabBadge::Dot(_)) if !dot_text => return label().into(),
        Some(TabBadge::Dot(color)) => {
            append(
                egui::RichText::new("\u{25CF} ")
                    .size(tokens::SIZE_CAPTION)
                    .color(color),
            );
            append(label());
        }
        Some(TabBadge::Count(count, role)) => {
            append(label());
            append(
                egui::RichText::new(format!(" {}", count_text(count)))
                    .size(size)
                    .color(role.text())
                    .strong(),
            );
        }
    }
    egui::WidgetText::LayoutJob(std::sync::Arc::new(job))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a strip painted.
    struct Painted {
        /// The number of distinct label rows.
        rows: usize,
        /// The largest label font size.
        size: f32,
        /// The width of the strip at full size.
        full: f32,
        /// The right edge of the rightmost painted text, from the strip's
        /// left edge.
        right: f32,
        /// The labels that painted, in x order.
        labels: Vec<String>,
        /// The dot badges that painted (filled circles in the dot colour).
        dots: usize,
        /// The dots whose ink touches the ink of a label.
        dots_on_labels: usize,
    }

    /// Draw a strip of `labels` at `width` with `labels[active]` active.
    /// Each `labels[dots[..]]` carries a dot badge and `labels[count]` a
    /// count badge, as the two inspectors draw them.
    fn draw(
        labels: &[&str],
        dots: &[usize],
        count: Option<usize>,
        active: usize,
        width: f32,
    ) -> Painted {
        let ctx = egui::Context::default();
        tokens::apply(&ctx);
        tokens::apply_fonts(&ctx);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        warmup.textures_delta.clear();
        let mut full = 0.0;
        let mut left = 0.0;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_max_width(width);
            left = ui.min_rect().left();
            let mut active = active;
            let strip =
                labels
                    .iter()
                    .enumerate()
                    .fold(TabStrip::new(&mut active), |s, (i, label)| {
                        let badge = if dots.contains(&i) {
                            Some(TabBadge::Dot(tokens::DANGER))
                        } else if Some(i) == count {
                            Some(TabBadge::Count(3, Role::Danger))
                        } else {
                            None
                        };
                        s.tab_badge(i, label, badge)
                    });
            full = full_width(ui, &strip.tabs);
            let _ = strip.show(ui);
        });
        let mut rows: Vec<f32> = Vec::new();
        let mut size = 0.0;
        let mut right: f32 = 0.0;
        let mut painted: Vec<(f32, String)> = Vec::new();
        let mut dot_count = 0;
        let mut dot_rects = Vec::new();
        let mut label_rects = Vec::new();
        for clipped in &out.shapes {
            if let egui::epaint::Shape::Circle(circle) = &clipped.shape
                && circle.fill == tokens::DANGER
            {
                dot_count += 1;
                let bounds = clipped.shape.visual_bounding_rect();
                right = right.max(bounds.right() - left);
                dot_rects.push(bounds);
            }
            if let egui::epaint::Shape::Text(text) = &clipped.shape {
                let bounds = clipped.shape.visual_bounding_rect();
                right = right.max(bounds.right() - left);
                let y = bounds.center().y;
                if !rows.iter().any(|r| (r - y).abs() < 4.0) {
                    rows.push(y);
                }
                if let Some(l) = labels.iter().find(|l| text.galley.job.text.contains(*l)) {
                    painted.push((bounds.left(), (*l).to_owned()));
                    label_rects.push(bounds);
                    size = text
                        .galley
                        .job
                        .sections
                        .iter()
                        .map(|s| s.format.font_id.size)
                        .fold(size, f32::max);
                }
            }
        }
        painted.sort_by(|a, b| a.0.total_cmp(&b.0));
        out.textures_delta.clear();
        Painted {
            rows: rows.len(),
            size,
            full,
            right,
            labels: painted.into_iter().map(|(_, l)| l).collect(),
            dots: dot_count,
            dots_on_labels: dot_rects
                .iter()
                .filter(|d| label_rects.iter().any(|l| l.intersects(**d)))
                .count(),
        }
    }

    const TOOLPATH: [&str; 6] = ["Geometry", "Cut", "Heights", "Feeds", "Linking", "Dressup"];
    /// The R2 tab set (ruling 2026-10-30).
    const TOOLPATH_R2: [&str; 5] = ["Geometry", "Passes", "Feeds", "Heights", "Linking"];
    const SIM: [&str; 4] = ["Metrics", "Warnings", "Navigate", "Project"];

    /// stepGT: at the inner width of the 280-point inspector (264 points)
    /// the six toolpath tabs, with a dot on Feeds, are compact and on ONE
    /// line. At full size they need more than the width (non-vacuity).
    #[test]
    fn the_toolpath_tabs_fit_one_line_compact_gt() {
        let p = draw(&TOOLPATH, &[3], None, 0, 264.0);
        assert!(
            p.full > 264.0,
            "non-vacuity: full size fits ({:.1})",
            p.full
        );
        assert_eq!(p.size, tokens::SIZE_CAPTION, "compact labels");
        assert_eq!(
            p.rows, 1,
            "one line at 264 points (full size {:.1})",
            p.full
        );
        assert_eq!(p.labels.len(), 6, "every tab on the strip: {:?}", p.labels);
    }

    /// stepGT: a strip that fits at full size keeps the full size: the
    /// four Simulation tabs, with a count on Warnings, at the inner width
    /// of the 304-point Simulation inspector (288 points).
    #[test]
    fn a_strip_that_fits_keeps_the_full_size_gt() {
        let p = draw(&SIM, &[], Some(1), 0, 288.0);
        assert!(p.full <= 288.0, "the sim tabs need {:.1} points", p.full);
        assert_eq!(p.size, tokens::SIZE_BODY);
        assert_eq!(p.rows, 1);
    }

    /// W1 (F10): the strip never takes a second row. At 240, 280 and 340
    /// points, for the five-tab and the six-tab set, with and without a
    /// dot, and with the first or the last tab active: one row, no text
    /// past the width, and the active tab on the strip.
    #[test]
    fn the_strip_never_wraps_w1() {
        let sets: [&[&str]; 2] = [&TOOLPATH_R2, &TOOLPATH];
        for width in [240.0_f32, 280.0, 340.0] {
            for labels in sets {
                let every: Vec<usize> = (0..labels.len()).collect();
                let dot_sets: [&[usize]; 4] = [&[], &[2], &[labels.len() - 1], &every];
                for dots in dot_sets {
                    for active in [0, labels.len() - 1] {
                        let p = draw(labels, dots, None, active, width);
                        let case = format!(
                            "{} tabs at {width} (dots {dots:?}, active {active}): {:?}",
                            labels.len(),
                            p.labels
                        );
                        assert_eq!(p.rows, 1, "{case}");
                        assert!(p.right <= width + 0.5, "{case}: right edge {:.1}", p.right);
                        let want = labels.get(active).copied().unwrap_or_default();
                        assert!(p.labels.iter().any(|l| l == want), "{case}: no active tab");
                    }
                }
            }
        }
        // W3 fix: at the 340-point default the five R2 tabs keep the full
        // size with a dot on every tab. A dot adds no width. Red proof:
        // the dot was a text run before the label, and one dot pushed the
        // strip into the compact form.
        let every: Vec<usize> = (0..TOOLPATH_R2.len()).collect();
        let bare = draw(&TOOLPATH_R2, &[], None, 0, 340.0);
        let dotted = draw(&TOOLPATH_R2, &every, None, 0, 340.0);
        assert!(dotted.full <= 340.0, "the R2 tabs need {:.1}", dotted.full);
        assert_eq!(
            dotted.size,
            tokens::SIZE_BODY,
            "five dots at 340: full size"
        );
        assert_eq!(dotted.rows, 1);
        assert_eq!(
            dotted.labels.len(),
            TOOLPATH_R2.len(),
            "{:?}",
            dotted.labels
        );
        assert_eq!(
            dotted.dots,
            TOOLPATH_R2.len(),
            "non-vacuity: each dot paints"
        );
        assert_eq!(dotted.dots_on_labels, 0, "each dot sits in the padding");
        assert!(
            (dotted.full - bare.full).abs() < 0.01,
            "a dot adds no width: {:.1} vs {:.1}",
            dotted.full,
            bare.full
        );
    }

    /// W1: when even the compact tabs do not fit, the tail goes into the
    /// `»` menu and the head keeps strip order; an active tail tab is
    /// appended after the head. Non-vacuity: 160 points overflows.
    #[test]
    fn a_narrow_strip_moves_the_tail_into_a_menu_w1() {
        let p = draw(&TOOLPATH, &[4], None, 5, 160.0);
        assert_eq!(p.rows, 1);
        assert!(
            p.labels.len() < TOOLPATH.len(),
            "non-vacuity: {:?}",
            p.labels
        );
        assert_eq!(p.labels.last().map(String::as_str), Some("Dressup"));
        assert_eq!(p.labels.first().map(String::as_str), Some("Geometry"));
        assert!(p.right <= 160.5, "right edge {:.1}", p.right);
    }
}
