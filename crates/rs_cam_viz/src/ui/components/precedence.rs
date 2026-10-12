//! `PrecedenceField` — a per-op override over a project default, made legible.
//!
//! Component layer (IA cleanup, ARCHITECTURE.md §3.8; wired by W3.1). A spindle
//! RPM (and, later, any "default vs override" value) is stored as
//! `Option<u32>`: `None` follows the project default, `Some(v)` overrides it.
//! The pre-component widget hid the actual default behind a hardcoded constant
//! and a "(uses project default)" hint, so the operator could not see which
//! value would actually run (finding P1-005 / P2-006).
//!
//! `PrecedenceField` renders both: a checkbox to engage the override, the
//! editable value when engaged, and the inherited project default shown as
//! `〈 N 〉` in the [`ProvKind::Inherited`] colour — so the precedence ("which
//! wins") is always visible. It is grid-friendly (calls `end_row`).

use super::provenance::ProvKind;

pub struct PrecedenceField<'a> {
    label: &'a str,
    override_value: &'a mut Option<u32>,
    project_default: u32,
    suffix: &'a str,
    speed: f64,
    range: std::ops::RangeInclusive<u32>,
    tooltip: Option<&'a str>,
}

impl<'a> PrecedenceField<'a> {
    pub fn new(label: &'a str, override_value: &'a mut Option<u32>, project_default: u32) -> Self {
        Self {
            label,
            override_value,
            project_default,
            suffix: "",
            speed: 1.0,
            range: 0..=u32::MAX,
            tooltip: None,
        }
    }

    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = suffix;
        self
    }

    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = speed;
        self
    }

    pub fn range(mut self, range: std::ops::RangeInclusive<u32>) -> Self {
        self.range = range;
        self
    }

    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    /// Draw the row and call `ui.end_row()`. Returns `true` if the override
    /// state or the value changed.
    ///
    /// # The value column is one line
    ///
    /// IC1 (2026-10-13). The row shows the value that runs:
    ///
    /// - Not overridden: `[ ] 〈 N 〉 unit`, the inherited project default in
    ///   the [`ProvKind::Inherited`] colour.
    /// - Overridden: `[x] [value]`, and the project default is on the hover
    ///   of the value.
    ///
    /// The checkbox carries no word; its hover says what it overrides. The
    /// row used to wrap the default onto a second line in the 280-point
    /// inspector, and in the 240-point rail it widened the panel.
    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        super::value_row::label_cell(ui, self.label);
        ui.horizontal(|ui| {
            // The narrow gap keeps the overridden value inside the 240-point
            // rail beside the 116-point label column.
            ui.spacing_mut().item_spacing.x = crate::ui::tokens::SPACE_1;
            let mut active = self.override_value.is_some();
            let mut value = self.override_value.unwrap_or(self.project_default);

            let mut resp = ui.checkbox(&mut active, "");
            resp = resp.on_hover_text(
                self.tooltip
                    .unwrap_or("Tick to override the project default for this operation."),
            );
            if resp.changed() {
                changed = true;
            }

            if active {
                // The unit is its own dim text after the well, so the well
                // holds only the number and the row fits the rail.
                if ui
                    .add(
                        egui::DragValue::new(&mut value)
                            .speed(self.speed)
                            .range(self.range.clone()),
                    )
                    .on_hover_text(format!(
                        "This operation's value. The project default is \
                         \u{2329} {} \u{232A}{}.",
                        self.project_default, self.suffix
                    ))
                    .changed()
                {
                    changed = true;
                }
                ui.add(egui::Label::new(super::text::unit(self.suffix.trim())).truncate());
            } else {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!(
                            "\u{2329} {} \u{232A}{}",
                            self.project_default, self.suffix
                        ))
                        .small()
                        .color(ProvKind::Inherited.color()),
                    )
                    .truncate()
                    .show_tooltip_when_elided(false),
                )
                .on_hover_text(
                    "The project default. Tick the box to set a value for this operation.",
                );
            }

            if changed {
                *self.override_value = if active { Some(value) } else { None };
            }
        });
        ui.end_row();
        changed
    }
}
