//! The multi-tool finishing planner dialog — Phase U of
//! `planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md`.
//!
//! # What the operator is being asked
//!
//! Which drawer tools take part (§3.2), how finely the board should be carved
//! into regions, and — looking at the preview overlay — whether the territory
//! that produces is worth generating (§3.1). Nothing here writes to the
//! project until **Apply plan**; Preview is a pure read, and Close leaves the
//! project exactly as it found it.
//!
//! # The veto shape, copied
//!
//! `ui::optimize_project` is the precedent: a status enum (Loading / Failed /
//! Ready) rendered from cached state, a per-row control, and one explicit
//! Apply. The named anti-pattern is the strategy advisor — MCP-only, zero GUI
//! surface, no apply path — which is why this dialog exists at all rather than
//! the tier map being something only an agent can see.
//!
//! # Two knobs the operator asked for by name
//!
//! **Region coarseness** is one slider spanning "lots of small regions" ↔ "a
//! few large ones". It scales the derived close radius and min-island area
//! together; the raw dials sit behind the advanced flyout, and typing one
//! takes it out of the slider's hands (the flyout says so).
//!
//! **Overlap** is visible, not buried: it is how far the fine tool reaches
//! past its own territory to blend the seam, and the preview paints that band
//! in its own tint so the number has a picture.
//!
//! # No background field locking
//!
//! Every numeric control is a plain editable field. The Feeds tab's standing
//! rule applies here.

use rs_cam_core::rest_heatmap_mesh::{tier_fill_color, tier_overlap_color};
use rs_cam_core::session::MultitoolPreview;
use rs_cam_core::tier_islands::{COARSENESS_MAX, COARSENESS_MIN, TierIslandSet, TierIslands};

use super::{AppEvent, theme};
use crate::state::AppState;
use crate::state::multitool_planner::{
    MAX_PLAN_CELL_MM, MIN_PLAN_CELL_MM, MultitoolPlannerState, MultitoolPreviewStatus,
    PREVIEW_DEBOUNCE,
};
use crate::ui_command::{NoArgs, UiCommand};

/// Draw the planner dialog if it is open.
pub fn draw(ctx: &egui::Context, state: &mut AppState, events: &mut Vec<AppEvent>) {
    let lane_busy = state.is_optimizing();
    let Some(planner) = state.multitool_planner.as_mut() else {
        return;
    };
    if !planner.open {
        return;
    }

    let mut still_open = true;
    egui::Window::new("Plan multi-tool finishing")
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(700.0)
        .open(&mut still_open)
        .show(ctx, |ui| {
            draw_body(ui, planner, lane_busy, events);
        });

    if !still_open {
        events.push(AppEvent::Ui(UiCommand::CloseMultitoolPlanner(NoArgs)));
    }

    // Debounced re-preview. An island-only dial re-cuts the SAME memoised map,
    // so this is the cheap half of the loop — but only once the operator stops
    // dragging.
    if planner.dirty_since.is_some() {
        if planner.debounce_elapsed() && !lane_busy {
            planner.dirty_since = None;
            events.push(AppEvent::PreviewMultitoolPlan);
        } else {
            // Nothing else asks for this frame, so ask for it here, or the
            // debounce would only fire on the next unrelated repaint.
            ctx.request_repaint_after(PREVIEW_DEBOUNCE);
        }
    }
}

fn draw_body(
    ui: &mut egui::Ui,
    planner: &mut MultitoolPlannerState,
    lane_busy: bool,
    events: &mut Vec<AppEvent>,
) {
    // ui-string-columns: air either side of the separator in a panel
    // header, where the model name may itself contain spaces.
    let header = format!(
        "Model: {}   ·   setup #{}",
        planner.model_name, planner.setup_index
    );
    ui.label(egui::RichText::new(header).small().color(theme::TEXT_MUTED));
    ui.add_space(6.0);

    egui::ScrollArea::vertical()
        .max_height(460.0)
        .show(ui, |ui| {
            draw_tool_list(ui, planner);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);
            draw_main_dials(ui, planner);
            ui.add_space(6.0);
            draw_advanced(ui, planner);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);
            draw_status(ui, planner);
        });

    ui.add_space(8.0);
    draw_actions(ui, planner, lane_busy, events);
}

fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_owned())
            .small()
            .strong()
            .color(theme::TEXT_MUTED),
    );
    ui.add_space(2.0);
}

// ── Tool ladder ─────────────────────────────────────────────────────────

fn draw_tool_list(ui: &mut egui::Ui, planner: &mut MultitoolPlannerState) {
    section_heading(ui, "LADDER");
    ui.label(
        egui::RichText::new(
            "Tick the tools that take part. The chain runs coarse to fine, and the tip \
             radius below is what decides that order — never the shank.",
        )
        .small()
        .color(theme::TEXT_DIM),
    );
    ui.add_space(4.0);

    egui::Grid::new("multitool_planner_tools")
        .num_columns(4)
        .spacing([10.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("use").small().strong());
            ui.label(egui::RichText::new("tool").small().strong());
            ui.label(egui::RichText::new("tip radius").small().strong());
            ui.label(egui::RichText::new("strategy").small().strong())
                .on_hover_text(
                    "Which operation cuts this tool's tier. The tier's TERRITORY is \
                     the same whichever you pick — regions come from the tier map; \
                     this only chooses the toolpath type. Unified = the band mix \
                     (raster/scallop/waterline per slope). Scallop = one continuous \
                     ring spiral over the whole territory. Iso Scallop = per-point \
                     ring spacing with the spec-correct slope law — measured faster \
                     than the band mix at better coverage on ONE large organic \
                     territory; on a tier of many small islands each island gets its \
                     own ring cascade, which costs entry plunges.",
                );
            ui.end_row();
            for row in &mut planner.tools {
                ui.checkbox(&mut row.selected, "");
                ui.label(egui::RichText::new(row.name.clone()).small());
                ui.label(
                    egui::RichText::new(format!("R{:.2} mm", row.cusp_radius_mm))
                        .small()
                        .color(theme::TEXT_MUTED),
                );
                if row.selected {
                    use rs_cam_core::session::TierStrategy;
                    let label = match row.strategy {
                        TierStrategy::UnifiedFinish => "Unified (bands)",
                        TierStrategy::Scallop => "Scallop (rings)",
                        TierStrategy::IsoScallop => "Iso Scallop",
                    };
                    egui::ComboBox::from_id_salt(("tier_strategy", row.tool_id))
                        .selected_text(egui::RichText::new(label).small())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut row.strategy,
                                TierStrategy::UnifiedFinish,
                                "Unified (bands)",
                            );
                            ui.selectable_value(
                                &mut row.strategy,
                                TierStrategy::Scallop,
                                "Scallop (rings)",
                            );
                            ui.selectable_value(
                                &mut row.strategy,
                                TierStrategy::IsoScallop,
                                "Iso Scallop",
                            );
                        });
                } else {
                    ui.label(egui::RichText::new("—").small().color(theme::TEXT_MUTED));
                }
                ui.end_row();
            }
        });

    if let Some(reason) = planner.blocking_reason() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(reason).small().color(theme::WARNING));
    }
}

// ── Dials ───────────────────────────────────────────────────────────────

fn draw_main_dials(ui: &mut egui::Ui, planner: &mut MultitoolPlannerState) {
    section_heading(ui, "REGIONS");

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("many small")
                .small()
                .color(theme::TEXT_DIM),
        );
        let slider = ui.add(
            egui::Slider::new(&mut planner.coarseness, COARSENESS_MIN..=COARSENESS_MAX)
                .logarithmic(true)
                .show_value(false),
        );
        ui.label(
            egui::RichText::new("few large")
                .small()
                .color(theme::TEXT_DIM),
        );
        ui.label(egui::RichText::new(format!("{:.2}x", planner.coarseness)).small());
        if slider.changed() {
            planner.mark_island_dial_dirty();
        }
        slider.on_hover_text(
            "Region coarseness. Scales the merge radius and the minimum island area \
             together from each tier's own tool-derived baseline — linearly and \
             quadratically, because they are one length scale and its square. A raw dial \
             typed in Advanced is taken verbatim and ignores this.",
        );
    });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Overlap").small());
        let drag = ui.add(
            egui::DragValue::new(&mut planner.overlap_mm)
                .suffix(" mm")
                .speed(0.05)
                .range(0.0..=10.0),
        );
        if drag.changed() {
            planner.mark_island_dial_dirty();
        }
        drag.on_hover_text(
            "How far each fine tier reaches past its own territory into the coarser \
             tier's, so the seam blends two cusp patterns instead of butting them. The \
             preview paints this band in a lighter tint of the tier's colour. Every tier \
             is dialled to the same cusp height and the same stock-to-leave, which is what \
             makes the blend a blend and not a step.",
        );
    });

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Cusp height").small());
        ui.add(
            egui::DragValue::new(&mut planner.cusp_height_mm)
                .suffix(" mm")
                .speed(0.005)
                .range(0.001..=1.0),
        )
        .on_hover_text(
            "The surface finish every tier is dialled to. Each tier's stepover is derived \
             from it and that tier's own tip radius, so a coarse tool takes wider passes \
             for the same finish.",
        );

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Tolerance").small());
        let drag = ui.add(
            egui::DragValue::new(&mut planner.tolerance_mm)
                .suffix(" mm")
                .speed(0.005)
                .range(0.001..=1.0),
        );
        if drag.changed() {
            // A MAP dial: the held map is stale, so there is nothing for the
            // debounced island re-cut to re-cut.
            planner.dirty_since = None;
        }
        drag.on_hover_text(
            "How much residual a coarse tool may leave and still keep a cell. It is a \
             RESIDUAL threshold, not a cusp height — a sane value sits above the coarse \
             tier's own cusp. Changing it is a fresh grid walk.",
        );
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut planner.coarse_skips_fine_islands,
            egui::RichText::new("Coarse tool skips fine islands").small(),
        )
        .on_hover_text(
            "Tier 0 leaves the fine tiers' islands uncut instead of sweeping the whole \
             board — a finer tool re-finishes them anyway. Saves the coarse pass that \
             share of its runtime, but the fine tools then meet the ROUGHING terraces \
             inside their islands instead of a coarse-finished surface: more load on \
             small cutters. The load gates measure it either way. Emission-only: the \
             preview's islands are unchanged by this.",
        );
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut planner.monotone_cell_decomposition,
            egui::RichText::new("Split shallow regions into monotone cells").small(),
        )
        .on_hover_text(
            "Each tier's SHALLOW band splits every region into monotone cells on that \
             region's own raster lattice, and rotates the lattice to the region's \
             PCA-minor axis where the region is elongated enough (gate 3.0). The passes \
             stay a raster and all cells share one lattice — per-cell directions, a cell \
             visit order and contour cells were each measured and are each SLOWER. \
             Measured on the reference relief under a realistic link ceiling: 1.155x \
             across the top three shallow regions, 1.215x on the elongated one. Those are \
             rig figures to approach, not promises, and cell seams change the cusp \
             pattern — eyeball the rendered surface before trusting a batch. \
             Emission-only: the preview's islands are unchanged by this.",
        );
    });
}

fn draw_advanced(ui: &mut egui::Ui, planner: &mut MultitoolPlannerState) {
    let title = egui::RichText::new("Advanced")
        .small()
        .color(theme::TEXT_MUTED);
    egui::CollapsingHeader::new(title)
        .id_salt("multitool_planner_advanced")
        .default_open(false)
        .show(ui, |ui| {
            draw_advanced_body(ui, planner);
        });
}

fn draw_advanced_body(ui: &mut egui::Ui, planner: &mut MultitoolPlannerState) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Planning cell").small());
        let drag = ui.add(
            egui::DragValue::new(&mut planner.cell_mm)
                .suffix(" mm")
                .speed(0.05)
                .range(MIN_PLAN_CELL_MM..=MAX_PLAN_CELL_MM),
        );
        if drag.changed() {
            planner.dirty_since = None;
        }
        drag.on_hover_text(
            "Grid resolution of the tier map, clamped to 0.2-1.0 mm. A 0.15 mm map costs \
             about 125 s PER LADDER TOOL on a 200 mm board, which is why the plan bans \
             that band outright. Changing this is a fresh grid walk.",
        );

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Margin").small());
        let drag = ui.add(
            egui::DragValue::new(&mut planner.margin_mm)
                .suffix(" mm")
                .speed(0.05)
                .range(0.0..=10.0),
        );
        if drag.changed() {
            planner.dirty_since = None;
        }
        drag.on_hover_text("Grid padding past the finest tool's envelope. A fresh grid walk.");
    });

    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(
            "The two dials below OVERRIDE the coarseness slider for whichever of them you \
             set — the core takes an explicit value verbatim at every coarseness. Clear \
             the tick to hand a dial back to the slider.",
        )
        .small()
        .color(theme::TEXT_DIM),
    );
    ui.add_space(4.0);

    optional_dial(
        ui,
        OptionalDial {
            label: "Merge radius (mm)",
            default_when_enabled: 0.5,
            max: 20.0,
            speed: 0.05,
            hover: "Islands closer together than this merge. Derived from the tier's own \
                    tip radius when unset.",
        },
        &mut planner.close_radius_mm,
        &mut planner.dirty_since,
    );
    optional_dial(
        ui,
        OptionalDial {
            label: "Min island (mm2)",
            default_when_enabled: 16.0,
            max: 10_000.0,
            speed: 1.0,
            hover: "An island under this area falls back to the COARSER tool rather than \
                    being dropped from the job. Derived from the tier's own tip radius \
                    when unset.",
        },
        &mut planner.min_region_area_mm2,
        &mut planner.dirty_since,
    );

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Max islands / tier").small());
        let drag = ui.add(
            egui::DragValue::new(&mut planner.max_regions_per_tier)
                .speed(1.0)
                .range(1..=256),
        );
        if drag.changed() {
            planner.mark_island_dial_dirty();
        }
        drag.on_hover_text(
            "Over this, the merge radius is auto-raised up to three times and then the \
             largest islands are kept. The preview says loudly when that happens.",
        );

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Rim erosion").small());
        let drag = ui.add(
            egui::DragValue::new(&mut planner.rim_erosion_mm)
                .suffix(" mm")
                .speed(0.1)
                .range(0.0..=20.0),
        );
        if drag.changed() {
            planner.mark_island_dial_dirty();
        }
        drag.on_hover_text(
            "Hands fine-tier cells within this distance of the part edge back to the \
             coarse tool. Off by default: near the edge a big tool hangs off and reads a \
             false-high residual, so the rim can look like fine territory. Roughly the \
             coarsest tool's ENVELOPE radius is what that band is worth.",
        );
    });
}

/// Everything one optional override dial needs except its value — a struct
/// rather than five more positional arguments, which is how a `max` and a
/// `speed` end up swapped.
#[derive(Clone, Copy)]
struct OptionalDial {
    label: &'static str,
    /// Value the dial takes the moment its tick is set. Not a default in the
    /// core sense: unticked means *derive it*, which no number can express.
    default_when_enabled: f64,
    max: f64,
    speed: f64,
    hover: &'static str,
}

/// One `Option<f64>` override: a tick that turns it on, and the value.
/// Unticked is `None` — "derive it, and scale it by the coarseness slider".
fn optional_dial(
    ui: &mut egui::Ui,
    dial: OptionalDial,
    value: &mut Option<f64>,
    dirty_since: &mut Option<std::time::Instant>,
) {
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        if ui.checkbox(&mut enabled, "").changed() {
            *value = enabled.then_some(dial.default_when_enabled);
            *dirty_since = Some(std::time::Instant::now());
        }
        ui.label(egui::RichText::new(dial.label).small());
        match value {
            Some(v) => {
                let drag = ui.add(
                    egui::DragValue::new(v)
                        .speed(dial.speed)
                        .range(0.0..=dial.max),
                );
                if drag.changed() {
                    *dirty_since = Some(std::time::Instant::now());
                }
                drag.on_hover_text(dial.hover);
            }
            None => {
                ui.label(
                    egui::RichText::new("(from coarseness)")
                        .small()
                        .color(theme::TEXT_DIM),
                )
                .on_hover_text(dial.hover);
            }
        }
    });
}

// ── Status / preview ────────────────────────────────────────────────────

fn draw_status(ui: &mut egui::Ui, planner: &MultitoolPlannerState) {
    match &planner.status {
        MultitoolPreviewStatus::Idle => {
            ui.label(
                egui::RichText::new(
                    "Press Preview to see which tool claims which part of the surface. \
                     Nothing is generated and nothing is changed until you press Apply.",
                )
                .small()
                .color(theme::TEXT_MUTED),
            );
        }
        MultitoolPreviewStatus::Loading => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(egui::RichText::new("Walking the surface…").strong());
            });
            ui.label(
                egui::RichText::new(
                    "One drop-cutter pass per ladder tool over the whole board: seconds at \
                     0.6 mm, tens of seconds at 0.3 mm. The GUI stays responsive — Close \
                     stops it.",
                )
                .small()
                .color(theme::TEXT_MUTED),
            );
        }
        MultitoolPreviewStatus::Failed(message) => {
            ui.label(
                egui::RichText::new("Preview failed")
                    .strong()
                    .color(theme::ERROR),
            );
            ui.label(egui::RichText::new(message.clone()).small());
        }
        MultitoolPreviewStatus::Ready(preview) => draw_ready(ui, planner, preview),
    }

    if let Some(error) = planner.apply_error.as_ref() {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(format!("Apply refused: {error}"))
                .small()
                .color(theme::ERROR),
        );
    }
}

fn draw_ready(ui: &mut egui::Ui, planner: &MultitoolPlannerState, preview: &MultitoolPreview) {
    ui.horizontal(|ui| {
        section_heading(ui, "TERRITORY");
        ui.add_space(8.0);
        let (hint, color) = if planner.map_is_cached() {
            ("map: cached", theme::TEXT_DIM)
        } else {
            ("map: rebuilding on next Preview", theme::WARNING)
        };
        ui.label(egui::RichText::new(hint).small().color(color))
            .on_hover_text(
                "Coarseness, overlap and the island dials re-cut the SAME map, which is \
                 why they are quick. Tolerance, cell, margin or a change of ladder need a \
                 fresh walk.",
            );
    });
    ui.add_space(4.0);

    egui::Grid::new("multitool_planner_tiers")
        .num_columns(7)
        .spacing([10.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("tier").small().strong());
            ui.label(egui::RichText::new("tool").small().strong());
            ui.label(egui::RichText::new("tip R").small().strong());
            ui.label(egui::RichText::new("islands").small().strong());
            ui.label(egui::RichText::new("raw").small().strong());
            ui.label(egui::RichText::new("owned area").small().strong());
            ui.label(egui::RichText::new("machines").small().strong());
            ui.end_row();
            for tier in 0..preview.map.tier_count {
                draw_tier_row(ui, preview, tier);
                ui.end_row();
            }
        });

    // The cap is the one place a count on this panel can be smaller than what
    // the plan actually found. It gets a loud row, not a footnote.
    for set in preview.islands.tiers_where_the_cap_acted() {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(cap_warning_text(set))
                .small()
                .strong()
                .color(theme::WARNING),
        );
    }

    // G-OVERLAPFILL. The band is doing what it is for, so this is an
    // advisory and not a refusal — but a fine tier that machines most of the
    // board is a time decision the operator has to make knowingly, and
    // nothing else on this panel says it.
    for advisory in preview.islands.band_advisories() {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(advisory.to_string())
                .small()
                .color(theme::WARNING),
        );
    }

    if preview.islands.is_empty() {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "No fine tier kept an island — the coarse tool holds the whole board at \
                 this tolerance. That is a planning outcome, not an error: raise the \
                 tolerance bar, lower the minimum island, or change the ladder.",
            )
            .small()
            .color(theme::WARNING),
        );
    }
}

/// One row of the per-tier table. Tier 0 has no island set by construction —
/// it is the coarse tool's complement and sweeps its territory as one pass —
/// so its island columns read "sweeps" rather than a misleading zero.
fn draw_tier_row(ui: &mut egui::Ui, preview: &MultitoolPreview, tier: usize) {
    let label = u8::try_from(tier).unwrap_or(u8::MAX);
    draw_swatch(ui, label);

    let name = preview
        .tool_names
        .get(tier)
        .cloned()
        .unwrap_or_else(|| format!("tier {tier}"));
    ui.label(egui::RichText::new(name).small());

    let cusp = preview.cusp_radii_mm.get(tier).copied().unwrap_or(0.0);
    ui.label(egui::RichText::new(format!("R{cusp:.2}")).small());

    match preview.islands.set_for_tier(label) {
        Some(set) => {
            ui.label(egui::RichText::new(set.islands.to_string()).small());
            ui.label(
                egui::RichText::new(set.raw_island_count.to_string())
                    .small()
                    .color(theme::TEXT_DIM),
            )
            .on_hover_text(
                "Connected components of this tier's raw labels, before merging, the \
                 minimum-island filter and the cap. A big gap between this and the kept \
                 count is the morphology doing its job.",
            );
            let area = format!("{:.0} mm2", set.owned_area_mm2);
            ui.label(egui::RichText::new(area).small());

            // G-OVERLAPFILL: the tool sweeps the islands PLUS the overlap
            // band, and on a dendritic map the band is most of it.
            let machines = match set.machining_to_owned_ratio() {
                Some(r) => format!("{:.0} mm2 ({r:.2}x)", set.machining_area_mm2),
                None => format!("{:.0} mm2", set.machining_area_mm2),
            };
            let over = set.band_advisory().is_some();
            let colour = if over {
                theme::WARNING
            } else {
                theme::TEXT_MUTED
            };
            ui.label(egui::RichText::new(machines).small().color(colour))
                .on_hover_text(format!(
                    "What this tier's tool actually sweeps: its islands grown by the \
                     {:.2} mm overlap band, which reaches into the coarser tool's \
                     territory so the two cusp patterns blend. The band closes every \
                     hole narrower than twice the overlap, and seals a concave bay \
                     into a new one — holes here go from {} to {}.",
                    set.overlap_mm, set.owned_hole_count, set.machining_hole_count,
                ));
        }
        None => {
            ui.label(
                egui::RichText::new("sweeps")
                    .small()
                    .color(theme::TEXT_MUTED),
            )
            .on_hover_text(
                "The coarsest tier's cusp target holds everywhere it is not beaten by a \
                 finer tier, so it sweeps its territory as one pass and carries no \
                 islands.",
            );
            ui.label(egui::RichText::new("—").small().color(theme::TEXT_DIM));
            let area = format!("{:.0} mm2", preview.map.tier_area_mm2(tier));
            ui.label(egui::RichText::new(area).small()).on_hover_text(
                "Cells LABELLED for this tool, grid-quantised. A LOWER bound on what it \
                 actually sweeps: every fine-tier island the minimum-island filter absorbed \
                 falls back here and is not counted in this number.",
            );
            ui.label(egui::RichText::new("sweeps").small().color(theme::TEXT_DIM))
                .on_hover_text(
                    "The coarse tool carries no overlap band — it holds the complement, \
                     and the fine tiers' bands reach into it.",
                );
        }
    }
}

/// A colour chip matching the overlay, plus the tier index. Reads
/// [`tier_fill_color`] — the exact function that colours the 3D mesh — so the
/// legend cannot drift from what is drawn.
fn draw_swatch(ui: &mut egui::Ui, tier: u8) {
    ui.horizontal(|ui| {
        let size = egui::vec2(10.0, 10.0);
        let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
        if let Some(rgb) = tier_fill_color(tier) {
            ui.painter().rect_filled(rect, 2.0, to_color32(rgb));
        }
        ui.label(egui::RichText::new(tier.to_string()).small());
    });
}

fn to_color32(rgb: [f32; 3]) -> egui::Color32 {
    let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
    egui::Color32::from_rgb(channel(rgb[0]), channel(rgb[1]), channel(rgb[2]))
}

/// The one-line answer under the buttons: how many islands, how much
/// territory the fine tiers OWN, and how much they actually MACHINE once the
/// overlap band is grown (G-OVERLAPFILL).
///
/// Pure, for the same reason [`cap_warning_text`] is: the wording is what an
/// operator reads, so a test has to be able to witness it.
///
/// The machining half is omitted when the band is off — then the two numbers
/// are the same and a second figure would only read as noise.
fn summary_line(islands: &TierIslands) -> String {
    let owned = islands.total_owned_area_mm2();
    let machining = islands.total_machining_area_mm2();
    let head = format!(
        "{} island(s) over {} fine tier(s), {owned:.0} mm2 of fine territory",
        islands.total_islands(),
        islands.per_tier.len(),
    );
    if owned <= 0.0 || (machining - owned).abs() < 0.5 {
        return head;
    }
    format!(
        "{head}, {machining:.0} mm2 machined with the overlap band ({:.2}x)",
        machining / owned
    )
}

/// What the per-tier cap did, in one operator-facing line.
///
/// Pure so the WORDING is testable: this crate has no egui render harness, so
/// splitting the derivation out is how a test can witness what the row says
/// rather than only that it drew something (the G-EXPL-HIDDEN lesson).
fn cap_warning_text(set: &TierIslandSet) -> String {
    let cap = &set.cap;
    let mut parts: Vec<String> = Vec::new();
    if cap.absorbed_by_min_area() > 0 {
        parts.push(format!(
            "{} island(s) under the {:.0} mm2 minimum went back to the coarser tool",
            cap.absorbed_by_min_area(),
            set.min_region_area_mm2
        ));
    }
    if cap.close_raises > 0 {
        parts.push(format!(
            "the merge radius was raised {}x to {:.2} mm to get under the cap of {}",
            cap.close_raises, cap.final_close_radius_mm, cap.cap
        ));
    }
    if cap.truncated() {
        parts.push(format!(
            "and {} island(s) were still DROPPED — the {} largest of {} were kept",
            cap.dropped(),
            cap.kept,
            cap.islands_after_min_area
        ));
    }
    if parts.is_empty() {
        return format!("Tier {}: the cap acted.", set.tier);
    }
    format!("Tier {}: {}.", set.tier, parts.join("; "))
}

// ── Actions ─────────────────────────────────────────────────────────────

fn draw_actions(
    ui: &mut egui::Ui,
    planner: &MultitoolPlannerState,
    lane_busy: bool,
    events: &mut Vec<AppEvent>,
) {
    let blocked = planner.blocking_reason();
    let can_preview = blocked.is_none() && !lane_busy;
    let can_apply = blocked.is_none() && planner.ready_preview().is_some() && !lane_busy;

    ui.horizontal(|ui| {
        let preview_btn = ui.add_enabled(can_preview, egui::Button::new("Preview"));
        if preview_btn.clicked() {
            events.push(AppEvent::PreviewMultitoolPlan);
        }
        let disabled_hint = blocked
            .clone()
            .unwrap_or_else(|| "A compute lane is already busy.".to_owned());
        preview_btn.on_disabled_hover_text(disabled_hint);

        let apply_btn = ui.add_enabled(can_apply, egui::Button::new("Apply plan"));
        if apply_btn.clicked() {
            events.push(AppEvent::ApplyMultitoolPlan);
        }
        apply_btn.on_disabled_hover_text(
            "Preview first — applying a ladder nobody has looked at is what the preview \
             exists to prevent.",
        );

        let close_btn = ui.button("Close");
        if close_btn.clicked() {
            events.push(AppEvent::Ui(UiCommand::CloseMultitoolPlanner(NoArgs)));
        }
        close_btn.on_hover_text(
            "Leaves the project untouched. Your ladder and dials are kept for next time.",
        );

        if let Some(preview) = planner.ready_preview() {
            ui.add_space(12.0);
            let summary = summary_line(&preview.islands);
            ui.label(
                egui::RichText::new(summary)
                    .small()
                    .color(theme::TEXT_MUTED),
            );
        }
    });

    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let size = egui::vec2(10.0, 10.0);
        let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
        let band = tier_overlap_color(1).unwrap_or([0.5, 0.5, 0.5]);
        ui.painter().rect_filled(rect, 2.0, to_color32(band));
        ui.label(
            egui::RichText::new("the lighter tint of a tier's colour is its overlap band")
                .small()
                .color(theme::TEXT_DIM),
        );
    });
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    use rs_cam_core::region_set::RegionSet;
    use rs_cam_core::tier_islands::TierCapReport;

    fn set_with(cap: TierCapReport, min_region_area_mm2: f64) -> TierIslandSet {
        TierIslandSet {
            tier: 1,
            islands: cap.kept,
            raw_island_count: 566,
            owned: RegionSet::new(Vec::new()),
            machining: RegionSet::new(Vec::new()),
            owned_area_mm2: 8_815.0,
            machining_area_mm2: 8_815.0,
            owned_hole_count: 0,
            machining_hole_count: 0,
            median_owned_hole_area_mm2: None,
            overlap_mm: 0.0,
            owned_cells: 0,
            owned_mask: Vec::new(),
            cap,
            close_radius_mm: cap.final_close_radius_mm,
            min_region_area_mm2,
        }
    }

    /// The truncating case is the one the operator MUST be told about: the
    /// island count on the panel is smaller than what the plan found, and
    /// nothing else on the surface says so.
    #[test]
    fn a_truncating_cap_names_the_dropped_islands() {
        let text = cap_warning_text(&set_with(
            TierCapReport {
                islands_after_close: 566,
                islands_after_min_area: 80,
                kept: 24,
                cap: 24,
                close_raises: 3,
                first_close_radius_mm: 0.5,
                final_close_radius_mm: 1.6875,
            },
            16.0,
        ));
        assert!(text.contains("DROPPED"), "got: {text}");
        assert!(text.contains("56 island(s)"), "the count, got: {text}");
        assert!(text.contains("raised 3x"), "the merge raise, got: {text}");
        assert!(text.contains("486 island(s)"), "absorbed, got: {text}");
    }

    /// A merge-only action is still an action — the islands the operator is
    /// looking at were welded together, and the radius that did it is stated
    /// rather than implied.
    #[test]
    fn a_merging_cap_states_the_radius_actually_in_force() {
        let text = cap_warning_text(&set_with(
            TierCapReport {
                islands_after_close: 30,
                islands_after_min_area: 20,
                kept: 20,
                cap: 24,
                close_raises: 1,
                first_close_radius_mm: 0.5,
                final_close_radius_mm: 0.75,
            },
            16.0,
        ));
        assert!(!text.contains("DROPPED"), "nothing was dropped: {text}");
        assert!(text.contains("0.75 mm"), "got: {text}");
    }

    fn islands_with(owned_area_mm2: f64, machining_area_mm2: f64) -> TierIslands {
        let mut set = set_with(
            TierCapReport {
                islands_after_close: 10,
                islands_after_min_area: 10,
                kept: 10,
                cap: 24,
                close_raises: 0,
                first_close_radius_mm: 0.5,
                final_close_radius_mm: 0.5,
            },
            16.0,
        );
        set.owned_area_mm2 = owned_area_mm2;
        set.machining_area_mm2 = machining_area_mm2;
        TierIslands {
            per_tier: vec![set],
            cell_mm: 0.4,
            tier_count: 2,
        }
    }

    /// G-OVERLAPFILL on the operator's one-line summary. The wanaka reading
    /// (owned 12 224, machining 29 954) has to say BOTH numbers and the
    /// ratio: an operator told only "12 224 mm2 of fine territory" has no way
    /// to know the fine tool sweeps 75 % of the board.
    #[test]
    fn the_summary_names_owned_and_machined_territory() {
        let text = summary_line(&islands_with(12_224.0, 29_954.0));
        assert!(text.contains("12224 mm2 of fine territory"), "got: {text}");
        assert!(text.contains("29954 mm2 machined"), "got: {text}");
        assert!(text.contains("2.45x"), "got: {text}");
    }

    /// With the band off the two areas are one number, and printing it twice
    /// would read as noise rather than as information.
    #[test]
    fn the_summary_drops_the_band_half_when_the_band_changes_nothing() {
        let text = summary_line(&islands_with(8_815.0, 8_815.0));
        assert!(text.contains("8815 mm2 of fine territory"), "got: {text}");
        assert!(!text.contains("machined"), "got: {text}");
    }

    /// Every fine tier gets a colour, and it is the colour the 3D overlay
    /// paints — the legend reads the same function the mesh does.
    #[test]
    fn the_swatch_colour_is_the_overlay_colour() {
        let fill = tier_fill_color(1).expect("tier 1 has a fill");
        let band = tier_overlap_color(1).expect("tier 1 has a band tint");
        assert_ne!(to_color32(fill), to_color32(band));
        assert_eq!(tier_fill_color(0), None, "tier 0 draws nothing");
    }
}
