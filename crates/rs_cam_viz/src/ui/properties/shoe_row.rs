//! H2 (`planning/op_architecture_2026-10-14/PLAN.md`, item H): the dust
//! shoe row of the toolpath Heights tab.
//!
//! The row writes nothing. A change is an `AppEvent::SetToolpathShoe`, which
//! the controller turns into one `Command::SetToolpathShoe` and one undo
//! step. The texts come from the core shoe plan through
//! [`super::ShoeRowInputs`]; the `inspect_machine` MCP tool prints the same
//! plan.

use rs_cam_core::machine::domain::HeadDimension;
use rs_cam_core::machine::head::ShoeSetting;

use super::ShoeRowInputs;
use crate::state::toolpath::ToolpathId;
use crate::ui::AppEvent;
use crate::ui::components::{ChoiceRow, PropRow};

/// The four settings, as the row's closed choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShoeMode {
    Auto,
    Gap,
    Raised,
    Off,
}

impl ShoeMode {
    fn of(setting: ShoeSetting) -> Self {
        match setting {
            ShoeSetting::Auto => Self::Auto,
            ShoeSetting::Gap { .. } => Self::Gap,
            ShoeSetting::Raised => Self::Raised,
            ShoeSetting::Off => Self::Off,
        }
    }
}

const MODE_OPTIONS: &[(ShoeMode, &str)] = &[
    (ShoeMode::Auto, "Auto"),
    (ShoeMode::Gap, "Gap"),
    (ShoeMode::Raised, "Raised"),
    (ShoeMode::Off, "Off"),
];

const MODE_HOVER: &str = "The dust shoe for this toolpath. You set the shoe at the tool \
     change; the run sheet in Readiness lists each setting in run order.\nAuto: the shoe \
     solver gives the lowest gap that keeps the hard shoe parts clear of the stock and \
     the obstacles.\nGap: you state the gap from the bit end up to the brush end.\n\
     Raised: the shoe is raised clear.\nOff: the shoe is removed.\nThe shoe moves no \
     toolpath. A change drops the simulation.";

const GAP_HOVER: &str = "The gap from the bit end up to the brush end. A gap below the \
     solver gap puts the shoe plate on the stock or an obstacle: the simulation reports \
     the strike, and the footer shows a Caution.";

/// The gap a switch to Gap starts from: the solver gap of the run, else
/// the shoe's normal gap.
fn seed_gap(shoe: &ShoeRowInputs) -> f64 {
    shoe.needed_gap_mm.unwrap_or(shoe.normal_gap_mm)
}

/// Draw the dust shoe row of toolpath `tp_id`.
pub(super) fn draw_shoe_row(
    ui: &mut egui::Ui,
    tp_id: ToolpathId,
    shoe: &ShoeRowInputs,
    events: &mut Vec<AppEvent>,
) {
    let mut mode = ShoeMode::of(shoe.setting);
    if ui
        .add(ChoiceRow::new("Dust shoe", &mut mode, MODE_OPTIONS).hover(MODE_HOVER))
        .changed()
    {
        let setting = match mode {
            ShoeMode::Auto => ShoeSetting::Auto,
            ShoeMode::Gap => ShoeSetting::Gap {
                gap_mm: seed_gap(shoe),
            },
            ShoeMode::Raised => ShoeSetting::Raised,
            ShoeMode::Off => ShoeSetting::Off,
        };
        events.push(AppEvent::SetToolpathShoe {
            tp_id,
            shoe: setting,
            merge: false,
        });
    }
    match shoe.setting {
        // W3d: the solver answer is the read-only form of the Shoe gap row.
        ShoeSetting::Auto => {
            let _ = PropRow::new("Shoe gap")
                .hover(Some(
                    "The shoe solver's answer for this toolpath's run. A run is the \
                     toolpaths with one tool and one shoe setting, one after the other.",
                ))
                .show_read_only(ui, shoe.auto_text.clone());
        }
        ShoeSetting::Gap { gap_mm } => {
            let mut gap = gap_mm;
            // W3d: the solver gap is on the hover of the row, not a line
            // under it.
            let hover = match shoe.needed_gap_mm {
                Some(needed) => format!(
                    "{GAP_HOVER}\n\nSolver: brush {needed:.0} mm above tip, the lowest gap \
                     that keeps the shoe plate clear for this run."
                ),
                None => GAP_HOVER.to_owned(),
            };
            // F210: a GUI drag range; `ShoeSetting::validate` reads the
            // domain. A value outside the range stays as it is.
            let out = PropRow::new("Shoe gap")
                .unit("mm")
                .hover(Some(&hover))
                .number(
                    ui,
                    &mut gap,
                    &crate::ui::components::NumberSpec::new(0.5)
                        .range(HeadDimension::ShoeGap.ui_range())
                        .keep_out_of_range(),
                );
            let response = &out.inner;
            if response.changed() && gap != gap_mm {
                events.push(AppEvent::SetToolpathShoe {
                    tp_id,
                    shoe: ShoeSetting::Gap { gap_mm: gap },
                    merge: response.dragged() && !response.drag_started(),
                });
            }
        }
        ShoeSetting::Raised | ShoeSetting::Off => {}
    }
}
