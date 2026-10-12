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
//! - `TabBadge::Dot` — a small dot in a role colour before the label.
//! - `TabBadge::Count` — the caution glyph and a count after the label, in
//!   the colour of its role. A count of 0 shows no badge.
//!
//! The strip wraps in a narrow rail (UR1: a tab past the panel edge is a
//! defect). The active tab takes `ACCENT_QUIET` and an `ACCENT` underline;
//! its radius is on the two top corners, because a tab joins the panel
//! below it (`DESIGN_SPEC.md` §2.2).

use crate::ui::components::Role;
use crate::ui::tokens;

/// The smallest width of a tab, in points. Below it a short label such as
/// "Dressup" wrapped to two rows.
const TAB_MIN_WIDTH: f32 = 55.0;

/// The glyph before the count of a count badge.
pub const COUNT_GLYPH: &str = "\u{26A0}";

/// The badge of one tab.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabBadge {
    /// A dot in this colour before the label.
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

    /// Draw the strip. Returns true when a click selected another tab.
    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        let active = self.active;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for tab in &self.tabs {
                let is_active = *active == tab.value;
                let (fill, text_color) = if is_active {
                    (tokens::ACCENT_QUIET, tokens::TEXT_STRONG)
                } else {
                    (egui::Color32::TRANSPARENT, tokens::TEXT_MUTED)
                };
                let button = egui::Button::new(tab_label(ui, tab, text_color))
                    .fill(fill)
                    .corner_radius(egui::CornerRadius {
                        nw: tokens::RADIUS_SM,
                        ne: tokens::RADIUS_SM,
                        sw: 0,
                        se: 0,
                    })
                    .min_size(egui::vec2(TAB_MIN_WIDTH, tokens::ROW_ACTION));
                let response = ui.add(button);
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
                ui.add_space(tokens::SPACE_1);
            }
        });
        changed
    }
}

/// The text of a count badge: the glyph and the count.
#[must_use]
pub fn count_text(count: usize) -> String {
    format!("{COUNT_GLYPH} {count}")
}

/// The label of one tab, with its badge, as one text run.
fn tab_label<T>(ui: &egui::Ui, tab: &Tab<'_, T>, text_color: egui::Color32) -> egui::WidgetText {
    let label = || {
        egui::RichText::new(tab.label)
            .size(tokens::SIZE_BODY)
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
                    .size(tokens::SIZE_BODY)
                    .color(role.text())
                    .strong(),
            );
        }
    }
    egui::WidgetText::LayoutJob(std::sync::Arc::new(job))
}
