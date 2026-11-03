//! `NotesCounter` — the quiet count of the notes of a panel (W1, P2).
//!
//! A note is not a problem, so it takes no row on the panel. The counter is
//! a small quiet button: the detail glyph (`GLYPH_DETAIL`) and the count. A
//! click opens a popover with one line per note face; the hover of a line
//! gives its detail. With no notes the counter draws nothing and takes no
//! space.

use crate::ui::{
    components::{Button, Notice},
    tokens,
};

/// The quiet count of a list of notes.
#[must_use]
pub struct NotesCounter<'a> {
    notes: &'a [Notice],
}

impl<'a> NotesCounter<'a> {
    pub fn new(notes: &'a [Notice]) -> Self {
        Self { notes }
    }

    /// The text of the button: the detail glyph and the count.
    #[must_use]
    pub fn text(count: usize) -> String {
        format!("{} {count}", tokens::GLYPH_DETAIL)
    }

    /// Draw the counter. `None` when there are no notes: nothing is drawn
    /// and no space is taken.
    pub fn show(self, ui: &mut egui::Ui) -> Option<egui::Response> {
        if self.notes.is_empty() {
            return None;
        }
        // The popover is no wider than the panel it opens from.
        let width = ui.available_width().max(tokens::LABEL_COL_WIDTH);
        let response = ui.add(Button::quiet(Self::text(self.notes.len())));
        let _ = egui::Popup::menu(&response).width(width).show(|ui| {
            ui.set_max_width(width);
            for note in self.notes {
                let line = ui.add(
                    egui::Label::new(
                        egui::RichText::new(&note.text)
                            .size(tokens::SIZE_BODY)
                            .color(tokens::TEXT_BODY),
                    )
                    .truncate(),
                );
                let hover = match note.detail.as_deref() {
                    Some(detail) if !detail.is_empty() => format!("{}\n\n{detail}", note.text),
                    _ => note.text.clone(),
                };
                let _ = line.on_hover_text(hover);
            }
        });
        Some(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::components::Role;

    /// Draw a counter of `notes`; returns whether it drew, the painted
    /// text runs and the cursor movement.
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
    )]
    fn draw(notes: &[Notice]) -> (bool, Vec<String>, f32) {
        let ctx = egui::Context::default();
        tokens::apply(&ctx);
        tokens::apply_fonts(&ctx);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        warmup.textures_delta.clear();
        let mut drew = false;
        let mut moved = 0.0;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let before = ui.cursor().top();
            drew = NotesCounter::new(notes).show(ui).is_some();
            moved = ui.cursor().top() - before;
        });
        let texts = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                // A `Button` also lays out a transparent copy of its label.
                egui::epaint::Shape::Text(t)
                    if t.galley
                        .job
                        .sections
                        .iter()
                        .any(|s| s.format.color != egui::Color32::TRANSPARENT) =>
                {
                    Some(t.galley.job.text.clone())
                }
                _ => None,
            })
            .collect();
        out.textures_delta.clear();
        (drew, texts, moved)
    }

    /// W1: no notes, no counter: nothing painted and no space taken.
    #[test]
    fn no_notes_draw_nothing_w1() {
        let (drew, texts, moved) = draw(&[]);
        assert!(!drew);
        assert!(texts.is_empty(), "{texts:?}");
        assert!(moved.abs() < 0.01, "the counter took {moved} points");
    }

    /// W1: with notes, the counter paints the glyph and the count once,
    /// and the faces stay in the closed popover.
    #[test]
    fn notes_draw_the_glyph_and_the_count_w1() {
        let notes = vec![
            Notice::new(Role::Info, "Two faces are below the floor").detail("Why."),
            Notice::new(Role::Info, "The boundary is the model outline"),
        ];
        let (drew, texts, moved) = draw(&notes);
        assert!(drew);
        assert!(moved > 0.0, "non-vacuity: the counter takes a row");
        let want = NotesCounter::text(2);
        assert_eq!(texts.iter().filter(|t| **t == want).count(), 1, "{texts:?}");
        assert!(
            !texts.iter().any(|t| t.starts_with("Two faces")),
            "a face painted on the panel: {texts:?}"
        );
    }
}
