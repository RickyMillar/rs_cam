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
//! Compact form (stepGT, operator ruling 2026-10-28): when the tabs at
//! their full size do not fit the width, the strip draws them compact:
//! caption-size labels, no minimum tab width, a 2-point padding and no
//! gap. The six toolpath tabs then fit one line of the 280-point
//! inspector. The labels do not change.
//!
//! The strip still wraps in a narrow rail (UR1: a tab past the panel edge
//! is a defect). The active tab takes `ACCENT_QUIET` and an `ACCENT` underline;
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
        let compact = full_width(ui, &self.tabs) > ui.available_width();
        ui.horizontal_wrapped(|ui| {
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
            for tab in &self.tabs {
                let is_active = *active == tab.value;
                let (fill, text_color) = if is_active {
                    (tokens::ACCENT_QUIET, tokens::TEXT_STRONG)
                } else {
                    (egui::Color32::TRANSPARENT, tokens::TEXT_MUTED)
                };
                let button = egui::Button::new(tab_label(ui, tab, text_color, size))
                    .fill(fill)
                    .corner_radius(egui::CornerRadius {
                        nw: tokens::RADIUS_SM,
                        ne: tokens::RADIUS_SM,
                        sw: 0,
                        se: 0,
                    })
                    .min_size(egui::vec2(min_width, tokens::ROW_ACTION));
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
                if !compact {
                    ui.add_space(tokens::SPACE_1);
                }
            }
        });
        changed
    }
}

/// The width of the strip at full size on one line: each tab's label and
/// padding, at least [`TAB_MIN_WIDTH`], and the gap after it.
fn full_width<T>(ui: &egui::Ui, tabs: &[Tab<'_, T>]) -> f32 {
    let padding = 2.0 * ui.spacing().button_padding.x;
    tabs.iter()
        .map(|tab| {
            let galley = tab_label(ui, tab, tokens::TEXT_STRONG, tokens::SIZE_BODY).into_galley(
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

/// The label of one tab, with its badge, as one text run.
fn tab_label<T>(
    ui: &egui::Ui,
    tab: &Tab<'_, T>,
    text_color: egui::Color32,
    size: f32,
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

    /// (rows, label font size, full width) of a strip of `labels` drawn at
    /// `width`. `labels[dot]` carries a dot badge and `labels[count]` a
    /// count badge, as the two inspectors draw them.
    fn strip(
        labels: &[&str],
        dot: Option<usize>,
        count: Option<usize>,
        width: f32,
    ) -> (usize, f32, f32) {
        let ctx = egui::Context::default();
        tokens::apply(&ctx);
        tokens::apply_fonts(&ctx);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        warmup.textures_delta.clear();
        let mut full = 0.0;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_max_width(width);
            let mut active = 0_usize;
            let strip =
                labels
                    .iter()
                    .enumerate()
                    .fold(TabStrip::new(&mut active), |s, (i, label)| {
                        let badge = if Some(i) == dot {
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
        for clipped in &out.shapes {
            if let egui::epaint::Shape::Text(text) = &clipped.shape
                && labels.iter().any(|l| text.galley.job.text.contains(l))
            {
                let y = clipped.shape.visual_bounding_rect().center().y;
                if !rows.iter().any(|r| (r - y).abs() < 4.0) {
                    rows.push(y);
                }
                size = text
                    .galley
                    .job
                    .sections
                    .iter()
                    .map(|s| s.format.font_id.size)
                    .fold(0.0, f32::max);
            }
        }
        out.textures_delta.clear();
        (rows.len(), size, full)
    }

    const TOOLPATH: [&str; 6] = ["Geometry", "Cut", "Heights", "Feeds", "Linking", "Dressup"];
    const SIM: [&str; 4] = ["Metrics", "Warnings", "Navigate", "Project"];

    /// stepGT: at the inner width of the 280-point inspector (264 points)
    /// the six toolpath tabs, with a dot on Feeds, are compact and on ONE
    /// line. At full size they need more than the width (non-vacuity).
    #[test]
    fn the_toolpath_tabs_fit_one_line_compact_gt() {
        let (rows, size, full) = strip(&TOOLPATH, Some(3), None, 264.0);
        assert!(full > 264.0, "non-vacuity: full size fits ({full:.1})");
        assert_eq!(size, tokens::SIZE_CAPTION, "compact labels");
        assert_eq!(rows, 1, "one line at 264 points (full size {full:.1})");
    }

    /// stepGT: a strip that fits at full size keeps the full size: the
    /// four Simulation tabs, with a count on Warnings, at the inner width
    /// of the 304-point Simulation inspector (288 points).
    #[test]
    fn a_strip_that_fits_keeps_the_full_size_gt() {
        let (rows, size, full) = strip(&SIM, None, Some(1), 288.0);
        assert!(full <= 288.0, "the sim tabs need {full:.1} points");
        assert_eq!(size, tokens::SIZE_BODY);
        assert_eq!(rows, 1);
    }
}
