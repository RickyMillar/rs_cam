//! `ListBox` — a list of items under the field rows (W3g, W3h).
//!
//! The operator, 2026-10-30: the selected cuts list "looks like it's part
//! of the controls, not a list of selected parts". A list box gives the
//! list its own surface:
//!
//! - an inset container across the panel width: the fill `LIST_FILL`, a
//!   1-point `HAIRLINE` border and the radius `RADIUS_SM`;
//! - a list header ([`list_header`]): the title at the left, then at the
//!   right the count, the state mark (a role glyph, its texts on hover)
//!   and the quiet acts;
//! - list rows ([`list_row`]): one line each, a hover highlight and a faint
//!   separator under each row; the label truncates, the acts sit at the
//!   right edge (W3h: the `⋯` menu, then `×`);
//! - a placeholder row ([`list_placeholder`]) while the list is empty;
//! - [`ListBox::rows`]: a long list scrolls inside the box, with a cap on
//!   the visible rows.
//!
//! A field row (`PropRow`) stays outside the box.

use crate::ui::components::{Button, Role};
use crate::ui::tokens;

/// The text of the menu act of a line (W3h).
pub const MENU: &str = "\u{22EF}";

/// The inset container of a list. See the module doc.
#[must_use]
pub struct ListBox {
    id_salt: egui::Id,
}

impl ListBox {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Self {
            id_salt: egui::Id::new(id_salt),
        }
    }

    /// Draw the box across the available width; `body` draws the header
    /// and the rows in it.
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        body: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        ui.push_id(self.id_salt, |ui| {
            egui::Frame::NONE
                .fill(tokens::LIST_FILL)
                .stroke(egui::Stroke::new(1.0, tokens::HAIRLINE))
                .corner_radius(tokens::RADIUS_SM)
                .inner_margin(egui::Margin::symmetric(
                    tokens::SPACE_2 as i8,
                    tokens::SPACE_1 as i8,
                ))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 0.0;
                    body(ui)
                })
                .inner
        })
    }

    /// W3h: draw `count` rows in a scroll area of at most `cap` visible
    /// rows. Only the visible rows are laid out (`show_rows`); `row`
    /// draws the row at an index. Call it in the body of [`Self::show`].
    pub fn rows(
        ui: &mut egui::Ui,
        id_salt: impl std::hash::Hash + std::fmt::Debug,
        count: usize,
        cap: usize,
        mut row: impl FnMut(&mut egui::Ui, usize),
    ) {
        let row_height = tokens::ROW_DENSE + ui.spacing().item_spacing.y;
        egui::ScrollArea::vertical()
            .id_salt(id_salt)
            .max_height(row_height * cap as f32)
            .auto_shrink([false, true])
            .show_rows(ui, row_height, count, |ui, range| {
                for index in range {
                    row(ui, index);
                }
            });
    }
}

/// What a list line reports: the index of the clicked button act, and
/// whether the pointer is over the line.
pub struct ListLine {
    pub clicked: Option<usize>,
    pub hovered: bool,
}

/// One act at the right edge of a line (W3h). A button reports its click
/// by its index in the act list; a menu draws its items in a popup.
pub enum Act<'a> {
    /// A quiet button: the label and the hover.
    Button(&'a str, &'a str),
    /// The `⋯` menu: the hover and the items. The items draw while the
    /// popup is open.
    Menu(&'a str, Box<dyn FnOnce(&mut egui::Ui) + 'a>),
}

/// The state mark of a list header (W3h): the role, and the texts that its
/// hover gives.
pub struct Mark<'a> {
    pub role: Role,
    pub hover: &'a str,
}

/// The header of a list: `title` at the left in `TEXT_STRONG`; at the right
/// the `count` in `TEXT_FAINT`, the state `mark` (W3h) and the `acts`.
pub fn list_header(
    ui: &mut egui::Ui,
    title: &str,
    count: &str,
    hover: &str,
    mark: Option<Mark<'_>>,
    acts: Vec<Act<'_>>,
) -> ListLine {
    line(
        ui,
        egui::RichText::new(title)
            .size(tokens::SIZE_CAPTION)
            .color(tokens::TEXT_STRONG),
        Some(count),
        mark,
        hover,
        acts,
        Style::Header,
    )
}

/// One row of a list: `text` in `color`, the `acts` at the right edge, the
/// hover highlight and a faint separator under the row. The text
/// truncates; `hover` gives the full text.
pub fn list_row(
    ui: &mut egui::Ui,
    text: &str,
    color: egui::Color32,
    hover: &str,
    acts: Vec<Act<'_>>,
) -> ListLine {
    line(
        ui,
        egui::RichText::new(text).color(color),
        None,
        None,
        hover,
        acts,
        Style::Row,
    )
}

/// The one row of an empty list (W3h): `text` in `TEXT_FAINT`, no act, no
/// highlight.
pub fn list_placeholder(ui: &mut egui::Ui, text: &str, hover: &str) {
    let _ = line(
        ui,
        egui::RichText::new(text).color(tokens::TEXT_FAINT),
        None,
        None,
        hover,
        Vec::new(),
        Style::Plain,
    );
}

/// A line with the list row layout and no surface of its own (no
/// highlight, no separator): a note line under a list, or a line in a
/// panel section. The text truncates; the buttons sit at the right edge.
pub fn list_line(
    ui: &mut egui::Ui,
    text: &str,
    color: egui::Color32,
    hover: &str,
    actions: &[(&str, &str)],
) -> ListLine {
    line(
        ui,
        egui::RichText::new(text).color(color),
        None,
        None,
        hover,
        actions
            .iter()
            .map(|(label, hover)| Act::Button(label, hover))
            .collect(),
        Style::Plain,
    )
}

/// The surface of a line.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    /// The list header: no highlight, no separator.
    Header,
    /// A list row: the hover highlight and the separator.
    Row,
    /// No surface.
    Plain,
}

/// One line of the box. FR-2 F22: the acts take their width first, at the
/// right edge; the text takes what is left and truncates.
fn line(
    ui: &mut egui::Ui,
    text: egui::RichText,
    count: Option<&str>,
    mark: Option<Mark<'_>>,
    hover: &str,
    acts: Vec<Act<'_>>,
    style: Style,
) -> ListLine {
    // The highlight paints under the line, so its slot comes first.
    let highlight = ui.painter().add(egui::Shape::Noop);
    let mut clicked = None;
    let line = ui.horizontal(|ui| {
        ui.set_min_height(tokens::ROW_DENSE);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Right to left: the last act sits at the right edge.
            let count_of_acts = acts.len();
            for (offset, act) in acts.into_iter().rev().enumerate() {
                let index = count_of_acts - 1 - offset;
                match act {
                    Act::Button(label, act_hover) => {
                        if ui
                            .add(Button::quiet(label))
                            .on_hover_text(act_hover)
                            .clicked()
                        {
                            clicked = Some(index);
                        }
                    }
                    Act::Menu(menu_hover, items) => {
                        let response = ui.add(Button::quiet(MENU)).on_hover_text(menu_hover);
                        let _ = egui::Popup::menu(&response).show(items);
                    }
                }
            }
            if let Some(mark) = mark {
                ui.label(
                    egui::RichText::new(mark.role.glyph().unwrap_or("\u{2022}"))
                        .color(mark.role.text()),
                )
                .on_hover_text(mark.hover);
            }
            if let Some(count) = count {
                ui.label(
                    egui::RichText::new(count)
                        .size(tokens::SIZE_CAPTION)
                        .color(tokens::TEXT_FAINT),
                );
            }
            let rest = ui.available_width().max(0.0);
            ui.allocate_ui_with_layout(
                egui::vec2(rest, tokens::ROW_DENSE),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_max_width(rest);
                    ui.add(
                        egui::Label::new(text)
                            .truncate()
                            .show_tooltip_when_elided(false),
                    )
                    .on_hover_text(hover);
                },
            );
        });
    });
    let rect = line.response.rect;
    let hovered = ui.rect_contains_pointer(rect);
    if hovered && style == Style::Row {
        ui.painter().set(
            highlight,
            egui::Shape::rect_filled(
                rect,
                egui::CornerRadius::from(tokens::RADIUS_SM),
                tokens::SURFACE_RAISED,
            ),
        );
    }
    if style == Style::Row {
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(1.0, tokens::HAIRLINE.gamma_multiply(0.5)),
        );
    }
    ListLine { clicked, hovered }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    /// W3g: the box paints its own inset surface (the list fill and the
    /// border) across the width, and a row paints a separator under it.
    /// W3h: the placeholder row paints no separator.
    #[test]
    fn a_list_box_has_its_own_surface_w3g() {
        let ctx = egui::Context::default();
        tokens::apply(&ctx);
        tokens::apply_fonts(&ctx);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        warmup.textures_delta.clear();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_max_width(300.0);
            let _ = ListBox::new("t").show(ui, |ui| {
                let _ = list_header(
                    ui,
                    "Picked faces",
                    "2",
                    "Picked faces (2)",
                    None,
                    Vec::new(),
                );
                let _ = list_row(
                    ui,
                    "Face Z20.00",
                    tokens::TEXT_BODY,
                    "Face",
                    vec![Act::Button("x", "Remove")],
                );
                let _ = list_row(ui, "Face Z12.00", tokens::TEXT_BODY, "Face", Vec::new());
                list_placeholder(ui, "Select faces", "Nothing is selected.");
            });
        });
        let boxes: Vec<egui::Rect> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::epaint::Shape::Rect(r)
                    if r.fill == tokens::LIST_FILL && r.stroke.color == tokens::HAIRLINE =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect();
        let separators = out
            .shapes
            .iter()
            .filter(|c| matches!(&c.shape, egui::epaint::Shape::LineSegment { .. }))
            .count();
        out.textures_delta.clear();
        assert_eq!(boxes.len(), 1, "one inset surface");
        assert!(boxes.first().unwrap().width() > 250.0, "{boxes:?}");
        assert_eq!(separators, 2, "one separator per row");
    }

    /// W3h: the acts of a row sit at the right edge in list order (the
    /// menu, then the button), and the state mark of a header paints its
    /// role glyph in the role colour.
    #[test]
    fn a_row_puts_the_menu_before_the_button_and_the_header_marks_its_state_w3h() {
        let ctx = egui::Context::default();
        tokens::apply(&ctx);
        tokens::apply_fonts(&ctx);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        warmup.textures_delta.clear();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_max_width(300.0);
            let _ = ListBox::new("t").show(ui, |ui| {
                let _ = list_header(
                    ui,
                    "Selection",
                    "3 loops",
                    "",
                    Some(Mark {
                        role: Role::Danger,
                        hover: "refused",
                    }),
                    Vec::new(),
                );
                let _ = list_row(
                    ui,
                    "Loop Z0 10.0 mm",
                    tokens::TEXT_BODY,
                    "",
                    vec![
                        Act::Menu("More", Box::new(|_ui| {})),
                        Act::Button("\u{2715}", "Remove"),
                    ],
                );
            });
        });
        let texts: Vec<(String, egui::Rect, egui::Color32)> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::epaint::Shape::Text(t) => Some((
                    t.galley.job.text.clone(),
                    c.shape.visual_bounding_rect(),
                    t.galley
                        .job
                        .sections
                        .first()
                        .map_or(egui::Color32::TRANSPARENT, |s| s.format.color),
                )),
                _ => None,
            })
            .collect();
        out.textures_delta.clear();
        let at = |s: &str| texts.iter().find(|(t, _, _)| t == s).unwrap();
        assert!(
            at(MENU).1.center().x < at("\u{2715}").1.center().x,
            "{texts:?}"
        );
        assert!(at("Loop Z0 10.0 mm").1.center().x < at(MENU).1.center().x);
        let glyph = Role::Danger.glyph().unwrap();
        assert!(
            texts
                .iter()
                .any(|(t, _, c)| t == glyph && *c == tokens::DANGER),
            "the mark paints in the role colour: {texts:?}"
        );
    }
}
