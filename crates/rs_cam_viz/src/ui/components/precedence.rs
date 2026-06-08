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

    /// Draw the row (`label : [☑ override] [value] · 〈 default 〉`) and call
    /// `ui.end_row()`. Returns `true` if the override state or value changed.
    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        ui.label(self.label);
        ui.horizontal(|ui| {
            let mut active = self.override_value.is_some();
            let mut value = self.override_value.unwrap_or(self.project_default);

            let mut resp = ui.checkbox(&mut active, "override");
            if let Some(tip) = self.tooltip {
                resp = resp.on_hover_text(tip);
            }
            if resp.changed() {
                changed = true;
            }

            if active {
                if ui
                    .add(
                        egui::DragValue::new(&mut value)
                            .suffix(self.suffix)
                            .speed(self.speed)
                            .range(self.range.clone()),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.label(
                    egui::RichText::new(format!(
                        "\u{00B7} default \u{2329} {} \u{232A}",
                        self.project_default
                    ))
                    .small()
                    .color(ProvKind::Inherited.color()),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!(
                        "\u{2329} {} \u{232A}{}",
                        self.project_default, self.suffix
                    ))
                    .small()
                    .color(ProvKind::Inherited.color()),
                )
                .on_hover_text("Project default — enable override to set a per-operation value.");
            }

            if changed {
                *self.override_value = if active { Some(value) } else { None };
            }
        });
        ui.end_row();
        changed
    }
}
