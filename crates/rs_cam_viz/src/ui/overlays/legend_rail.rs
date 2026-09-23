//! The legend rail: one compact block per active colour encoding, drawn above
//! the viewport dock (viewport redesign, MOCKUPS §2 and §6).
//!
//! The rail shows while the dock popovers and the catalogue are closed, so
//! the operator can always read what a colour means. It reads derived state
//! each frame. It writes no state and starts no compute.
//!
//! Two kinds of line exist:
//!
//! - [`RailLine::Scale`] — an encoding that had a scale legend in the old
//!   Overlays panel. The rail draws its name and the same scale.
//! - [`RailLine::NameOnly`] — an encoding that had no legend before the
//!   redesign. The rail draws its name only. The legend audit of phase 4
//!   gives each one its scale or its categories.

use crate::state::AppState;
use crate::state::selection::Selection;
use crate::state::viewport::ToolpathColorMode;
use crate::ui::tokens;

use super::registry::{self, Legend};

/// One block of the rail.
#[derive(Debug, Clone, PartialEq)]
pub enum RailLine {
    /// An encoding with a scale legend.
    Scale(Legend),
    /// An encoding with no legend yet. The rail draws the name only.
    NameOnly(NameOnly),
}

/// The active encodings that have no scale legend yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameOnly {
    /// The per-toolpath palette colour of the drawn toolpaths.
    ToolpathPalette,
    /// Cut, rapid and span-kind colours in Palette move colour.
    Moves,
    /// The entry-move markers of the selected toolpath.
    EntryMarkers,
    /// The Z planes of the selected toolpath.
    HeightPlanes,
    /// The collision markers and their density ramp.
    Collisions,
}

impl NameOnly {
    /// The name the rail draws.
    pub fn name(self) -> &'static str {
        match self {
            Self::ToolpathPalette => "Toolpaths",
            Self::Moves => "Moves",
            Self::EntryMarkers => "Entry markers",
            Self::HeightPlanes => "Height planes",
            Self::Collisions => "Collisions",
        }
    }
}

/// Is the row switched on AND able to draw now?
fn ready_on(state: &AppState, id: &str) -> bool {
    registry::row(id).is_some_and(|r| (r.get)(state) && (r.precondition)(state).is_ready())
}

/// Does the viewport draw at least one toolpath? The same rule as the GPU
/// upload and the pick: `toolpaths_to_draw`.
fn any_toolpath_drawn(state: &AppState) -> bool {
    !crate::state::viewport::toolpaths_to_draw(
        crate::state::viewport::ToolpathDrawFilter::from_state(state),
        state.session.toolpath_configs().iter().map(|tc| {
            let rt = state.gui.toolpath_rt.get(&tc.id);
            (
                tc.id,
                rt.is_none_or(|r| r.visible),
                rt.is_some_and(|r| r.result.is_some()),
            )
        }),
    )
    .is_empty()
}

/// Every active colour encoding, in rail order.
///
/// A scale line comes from [`registry::active_legends`], which needs the
/// flag on AND the precondition `Ready`. A name-only line uses the same
/// rule, so a row that cannot draw puts no line on the rail.
pub fn active_lines(state: &AppState) -> Vec<RailLine> {
    let mut out: Vec<RailLine> = registry::active_legends(state)
        .into_iter()
        .map(RailLine::Scale)
        .collect();
    let drawn = any_toolpath_drawn(state);
    let palette = matches!(
        state.viewport.toolpath_color_mode,
        ToolpathColorMode::Normal
    );
    if drawn && palette && (ready_on(state, "cutting_moves") || ready_on(state, "rapids")) {
        out.push(RailLine::NameOnly(NameOnly::ToolpathPalette));
        out.push(RailLine::NameOnly(NameOnly::Moves));
    }
    if drawn && ready_on(state, "entry_markers") {
        out.push(RailLine::NameOnly(NameOnly::EntryMarkers));
    }
    if ready_on(state, "height_planes") && matches!(state.selection, Selection::Toolpath(_)) {
        out.push(RailLine::NameOnly(NameOnly::HeightPlanes));
    }
    if ready_on(state, "collisions") {
        out.push(RailLine::NameOnly(NameOnly::Collisions));
    }
    out
}

/// `#n` for the selected toolpath, where `n` is the 1-based config index.
fn selected_number(state: &AppState) -> Option<usize> {
    let Selection::Toolpath(id) = state.selection else {
        return None;
    };
    state
        .session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.id == id)
        .map(|index| index + 1)
}

/// The target word of a move-colour legend: `#n` with `Paths: Selected`,
/// `all drawn` with `Paths: All`.
fn moves_target(state: &AppState) -> String {
    if state.viewport.show_all_toolpaths {
        "all drawn".to_owned()
    } else {
        selected_number(state).map_or_else(|| "none".to_owned(), |n| format!("#{n}"))
    }
}

/// The name of one scale legend (MOCKUPS §6).
fn scale_name(state: &AppState, legend: &Legend) -> String {
    let number = selected_number(state).map_or_else(String::new, |n| format!(" \u{00B7} #{n}"));
    match legend {
        Legend::RestHeatmap(..) => format!("Rest{number}"),
        Legend::Reach(_) => format!("Reach{number}"),
        Legend::TierMap(_) => match state.multitool_planner.as_ref() {
            Some(planner) => format!("Tier map \u{00B7} setup {}", planner.setup_index + 1),
            None => "Tier map".to_owned(),
        },
        Legend::Deviation => "Stock deviation".to_owned(),
        Legend::ByHeight => "Stock height".to_owned(),
        Legend::Engagement => format!("Move colour: Engagement \u{00B7} {}", moves_target(state)),
        Legend::AdvancePerTooth => {
            format!(
                "Move colour: Advance / tooth \u{00B7} {}",
                moves_target(state)
            )
        }
    }
}

/// Draw the rail when at least one encoding is active. Returns `true` when
/// it drew something.
pub fn draw(ui: &mut egui::Ui, state: &AppState) -> bool {
    let lines = active_lines(state);
    if lines.is_empty() {
        return false;
    }
    egui::Frame::default()
        .fill(tokens::SURFACE_RAISED)
        .corner_radius(tokens::RADIUS_MD)
        .inner_margin(egui::Margin::same(tokens::SPACE_3 as i8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = tokens::SPACE_1;
            for line in &lines {
                match line {
                    RailLine::Scale(legend) => {
                        name_label(ui, &scale_name(state, legend));
                        draw_legend(ui, state, *legend);
                    }
                    RailLine::NameOnly(name) => name_label(ui, name.name()),
                }
            }
        });
    true
}

fn name_label(ui: &mut egui::Ui, name: &str) {
    ui.label(
        egui::RichText::new(name)
            .size(tokens::SIZE_CAPTION)
            .color(tokens::TEXT_BODY),
    );
}

fn caption(text: impl Into<String>, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(text.into())
        .size(tokens::SIZE_CAPTION)
        .color(color)
}

// ── the scale legends ──────────────────────────────────────────────────────

/// A horizontal gradient bar plus its end labels.
///
/// `sample` maps `0..=1` along the bar to the colour the 3D overlay paints.
/// Every caller passes the overlay's OWN colour function, so a legend cannot
/// drift from what is drawn. The row wraps at a narrow width; it never
/// pushes the rail past the viewport.
fn gradient_strip(ui: &mut egui::Ui, left: &str, right: &str, sample: impl Fn(f32) -> [f32; 3]) {
    ui.horizontal_wrapped(|ui| {
        ui.label(caption(left, tokens::TEXT_MUTED));
        let (rect, _resp) = ui.allocate_exact_size(egui::vec2(96.0, 10.0), egui::Sense::hover());
        let painter = ui.painter();
        const SEGMENTS: u32 = 24;
        for i in 0..SEGMENTS {
            let t0 = i as f32 / SEGMENTS as f32;
            let t1 = (i + 1) as f32 / SEGMENTS as f32;
            let [r, g, b] = sample(t0);
            let seg = egui::Rect::from_min_max(
                egui::pos2(rect.left() + t0 * rect.width(), rect.top()),
                egui::pos2(rect.left() + t1 * rect.width(), rect.bottom()),
            );
            painter.rect_filled(seg, 0.0, rgb(r, g, b));
        }
        ui.label(caption(right, tokens::TEXT_MUTED));
    });
}

fn rgb(r: f32, g: f32, b: f32) -> egui::Color32 {
    tokens::from_linear_rgb([r, g, b])
}

/// A row of discrete labelled swatches, for a legend whose scale is a set of
/// classes rather than a continuum.
fn swatch_row(ui: &mut egui::Ui, title: &str, swatches: &[(&str, [f32; 3])]) {
    ui.label(caption(title, tokens::TEXT_MUTED));
    ui.horizontal_wrapped(|ui| {
        for (label, [r, g, b]) in swatches {
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(12.0, 10.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 0.0, rgb(*r, *g, *b));
            resp.on_hover_text(*label);
            ui.label(caption(*label, tokens::TEXT_MUTED));
        }
    });
}

fn draw_legend(ui: &mut egui::Ui, state: &AppState, legend: Legend) {
    match legend {
        Legend::RestHeatmap(threshold, peak) => {
            use rs_cam_core::maps::rest_heatmap_mesh::rest_ramp_color;
            let threshold = threshold as f32;
            let peak = peak.max(threshold + 1e-6);
            gradient_strip(
                ui,
                &format!("Rest {threshold:.2} mm"),
                &format!("{peak:.2} mm"),
                |t| rest_ramp_color(threshold + t * (peak - threshold), threshold, peak),
            );
        }
        Legend::Reach(ramp) => {
            use rs_cam_core::maps::reach_map::reach_color;
            // The strip draws the DEPTH band only — the bar to the deepest
            // gap, on the ramp's own log scale — because that is the part
            // with structure in it. Green and grey are single colours and
            // are named on the line under it rather than given ramp width.
            let bar = ramp.bar_mm();
            let top = ramp.max_gap_mm.max(bar);
            let ratio = (top / bar).max(1.0);
            gradient_strip(
                ui,
                &format!("miss {bar:.3} mm"),
                &format!("{top:.2} mm"),
                move |t| {
                    // Inverse of `ReachRamp::depth_t`, nudged past the bar so
                    // t = 0 samples the first MISS colour and not the green.
                    let gap = bar * ratio.powf(f64::from(t)) * 1.001;
                    reach_color(gap as f32, f32::NAN, ramp)
                },
            );
            ui.label(caption(
                format!(
                    "green \u{2264} {:.3} mm \u{00B7} grey = unresolved \u{00B7} mid {:.2} mm \u{00B7} log scale",
                    ramp.tolerance_mm,
                    ramp.mid_stop_mm(),
                ),
                tokens::TEXT_FAINT,
            ));
            // P5.3 - the moves are dimmed at DRAW TIME while the shading is
            // on, so the row still reads ON in the dock. Said here, because
            // an operator seeing Cutting moves ticked and faint lines on
            // screen would otherwise be looking at a contradiction.
            if state.viewport.show_cutting || state.viewport.show_rapids {
                ui.label(caption(
                    "moves dimmed while reach map is on",
                    tokens::TEXT_FAINT,
                ));
            }
            if let Some(map) = state.gui.reach_overlay.ready_map() {
                let measured = map.is_measured();
                ui.label(caption(
                    if measured {
                        format!(
                            "unreachable {:.1}% {} \u{00B7} worst gap {:.3} mm",
                            map.unreachable_pct(),
                            map.area_basis_note(),
                            map.max_gap_mm
                        )
                    } else {
                        "reach: not measured".to_owned()
                    },
                    tokens::TEXT_FAINT,
                ));
                // The grid, always, and on its own line: two percentages are
                // comparable only on one grid (F5, 2026-09-08). It also
                // carries the "the bar is under the floor" sentence, which is
                // what the wanaka red terrain needed said.
                if measured {
                    let below = map.tolerance_below_floor();
                    ui.label(caption(
                        map.grid_note(),
                        if below {
                            tokens::CAUTION
                        } else {
                            tokens::TEXT_FAINT
                        },
                    ));
                    if below {
                        ui.label(caption(map.over_statement_note(), tokens::CAUTION));
                    }
                }
            }
        }
        Legend::Deviation => {
            use crate::render::sim_render::deviation_colors;
            gradient_strip(ui, "Over-cut \u{2212}1 mm", "+1 mm remaining", |t| {
                let mm = -1.0 + 2.0 * t;
                deviation_colors(&[mm]).first().copied().unwrap_or([0.0; 3])
            });
        }
        Legend::ByHeight => {
            use rs_cam_core::export::ribbon::height_gradient_colors;
            // The function normalises over the vertices it is handed, so one
            // synthetic ramp of Z values reproduces the mesh's own scale.
            let mut vertices = Vec::with_capacity(3 * 25);
            for i in 0..25 {
                vertices.extend_from_slice(&[0.0, 0.0, i as f32 / 24.0]);
            }
            let colors = height_gradient_colors(&vertices);
            gradient_strip(ui, "Height low", "high", move |t| {
                let index = ((t * 24.0).round() as usize).min(24);
                colors.get(index).copied().unwrap_or([0.0; 3])
            });
        }
        Legend::Engagement => {
            use crate::render::toolpath_render::engagement_color;
            gradient_strip(ui, "Load heavy", "light", |t| {
                engagement_color(f64::from(t) * 1.5, 1.0)
            });
        }
        Legend::AdvancePerTooth => {
            use crate::render::toolpath_render::advance_per_tooth_segment_color;
            use rs_cam_core::feeds::{AdvancePerToothMm, VendorChiploadBand};
            // The band is per matched vendor row, so the legend labels the
            // CLASSES rather than absolute mm/tooth — and it reads them out
            // of the same classifier the lines use. A probe band supplies the
            // class boundaries; the RGB comes from the shipped function.
            let band = VendorChiploadBand::from_advance_range(&(0.05..0.10));
            let at = |mm: f64| {
                advance_per_tooth_segment_color(Some(&band), Some(AdvancePerToothMm::new(mm)))
            };
            swatch_row(
                ui,
                "Advance / tooth vs the matched vendor band",
                &[
                    ("below", at(0.01)),
                    ("within", at(0.075)),
                    ("near max", at(0.099)),
                    ("above", at(0.2)),
                    (
                        "no band",
                        advance_per_tooth_segment_color(None, Some(AdvancePerToothMm::new(0.05))),
                    ),
                ],
            );
        }
        Legend::TierMap(tier_count) => {
            use rs_cam_core::maps::rest_heatmap_mesh::tier_fill_color;
            ui.label(caption(
                "Tier map \u{2014} one colour per tool tier",
                tokens::TEXT_MUTED,
            ));
            ui.horizontal_wrapped(|ui| {
                // Tier 0 has no colour: it is the coarse tool's complement
                // and the overlay deliberately draws nothing there.
                for tier in 1..=tier_count {
                    let Ok(tier) = u8::try_from(tier) else {
                        break;
                    };
                    let Some([r, g, b]) = tier_fill_color(tier) else {
                        continue;
                    };
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(12.0, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, rgb(r, g, b));
                    resp.on_hover_text(format!("tier {tier}"));
                }
            });
        }
    }
}
