//! The Linking and Dressup tabs, and the parameter-grid helpers every
//! operation editor draws its rows with.
//!
//! `draw_linking_params` and `draw_dressup_params` call each other, so they
//! share one file.

use super::feeds_speeds::draw_entry_preview_diagram;
use super::operations::{draw_dogbone_diagram, draw_lead_in_out_diagram};
use super::{operations, pills};
use crate::state::toolpath::{DressupConfig, DressupEntryStyle, HeightContext, ToolpathEntry};
use crate::ui::automation;
use crate::ui::components::ValueRow;

// --- Parameter grid helpers ---

pub(super) fn dv(
    ui: &mut egui::Ui,
    label: &str,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
) {
    // Delegates to the shared `ValueRow` component (the one labelled-input row).
    let out = ValueRow::new(label, val, suffix, speed, range)
        .tooltip(tooltip_for(label))
        .show(ui);
    record_stock_to_leave(ui, label, &out);
}

/// G-DEPTHSTOCK (UX-R03-007): the caution row under a 2.5D depth field.
///
/// Rendered inside the operation's parameter grid, directly below the depth
/// row, in the same amber the diagnostics ribbon uses for a `Caution`. It is
/// the row form of the finding the header ribbon prints; both read
/// [`operations::depth_beyond_stock`]. Nothing is drawn when there is no
/// finding, so the grid keeps its shape.
pub(super) fn depth_caution_row(ui: &mut egui::Ui, caution: Option<&operations::DepthBeyondStock>) {
    let Some(caution) = caution else {
        return;
    };
    ui.label("");
    ui.label(
        egui::RichText::new(caution.message())
            .small()
            .color(crate::ui::tokens::CAUTION),
    )
    // F1.18: the last clause used to name the Bottom Z on the Heights tab as
    // a third thing to check. The rule no longer reads that field, and on
    // every operation this caution fires for the pin moves no motion at all
    // (F1.19), so naming it sent the operator to a dial that cannot fix this.
    .on_hover_text(format!(
        "The cut floor sits {:.2} mm below the bottom of a {:.2} mm board, so the \
         tool cuts into the bed. Generate stays enabled. Check the operation's \
         Depth field, or the stock thickness on the Stock panel.",
        caution.excess_mm, caution.stock_thickness_mm
    ));
    ui.end_row();
}

/// G-THROUGHCUT (UX-R03-006): the through-cut line under the Profile depth
/// field.
///
/// Informational, in the panel's own text colour: a through cut is the
/// normal way to cut a part out, and zero tabs is a valid holding choice.
/// The hover names the consequence (the last pass frees the part) and the
/// holding options. Drawn before [`depth_caution_row`], so a depth beyond
/// the board shows this line first and the amber caution under it. Nothing
/// is drawn for a partial-depth profile, so the grid keeps its shape.
pub(super) fn through_cut_row(ui: &mut egui::Ui, through_cut: Option<&operations::ThroughCut>) {
    let Some(through_cut) = through_cut else {
        return;
    };
    ui.label("");
    ui.label(egui::RichText::new(through_cut.message()).small())
        .on_hover_text(format!(
            "The cut bottom reaches the bottom of the {:.2} mm board, so the last pass \
             frees the part. Tabs hold it in the sheet until you cut them; zero tabs \
             is valid when a vacuum table or double-sided tape holds the part.",
            through_cut.stock_thickness_mm
        ));
    ui.end_row();
}

/// The "Stock to Leave" UI-automation hook, shared by `dv`/`dv_pill`.
fn record_stock_to_leave(
    ui: &mut egui::Ui,
    label: &str,
    out: &crate::ui::components::ValueRowOutcome,
) {
    if label.trim().trim_end_matches(':') == "Stock to Leave" {
        automation::record(
            ui,
            "properties_stock_to_leave",
            &out.value_response,
            "Stock to Leave",
        );
        automation::record(
            ui,
            "properties_stock_to_leave_label",
            &out.label_response,
            "Stock to Leave",
        );
    }
}

// PR-2D Phase 2 — per-field LUT Suggest pills.
//
// `dv_pill` is `dv` with an optional ⚡ Suggest pill rendered inline to the
// right of the DragValue. Both now delegate to the shared `ValueRow` /
// `SuggestButton` components; pill colour + source wording come from the one
// `ProvKind` vocabulary (green for a vendor LUT row, amber for the formula
// fallback or edge-radius floor) rather than the old local
// `pill_color_for_source` / `source_short_label` helpers.

/// Same as [`dv`] but with an optional inline ⚡ Suggest pill that pushes
/// the recommended value into the field on click. Delegates to [`ValueRow`]
/// with the [`Suggestion`] a [`PillSuggestions`] built for this field.
///
/// G-PILLCLAMP (2026-09-10): the suggestion is the apply funnel's as-applied
/// value for the field (or a labelled raw fallback when the funnel does not
/// write it), so the pill offers and writes the same number as `⚡ Apply cut
/// geometry`. A click is recorded on the `PillSuggestions` so the caller can
/// stamp the recommendation's provenance on the entry.
pub(super) fn dv_pill(
    ui: &mut egui::Ui,
    label: &str,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    suggestion: Option<pills::PillSuggestion<'_>>,
) -> bool {
    let mut row = ValueRow::new(label, val, suffix, speed, range).tooltip(tooltip_for(label));
    let mut click = None;
    if let Some(pill) = suggestion {
        let (s, c) = pill.into_parts();
        row = row.suggest(s);
        click = Some(c);
    }
    let out = row.show(ui);
    record_stock_to_leave(ui, label, &out);
    if out.suggested
        && let Some(c) = &click
    {
        c.record();
    }
    out.suggested
}

fn tooltip_for(label: &str) -> Option<&'static str> {
    Some(match label.trim().trim_end_matches(':') {
        "Stepover" => {
            "Distance between passes. 40-60% of diameter for roughing, 10-20% for finishing."
        }
        "Depth" => "Total cut depth from stock surface.",
        "Depth/Pass" | "Depth per Pass" => {
            "Max depth per Z level. Wood: 1-3mm small tools, up to half diameter for large."
        }
        "Feed Rate" => {
            "Cutting speed (mm/min). Wood: 500-2000 for small tools, 1500-4000 for large."
        }
        "Plunge Rate" => "Vertical feed speed (mm/min). Typically 30-50% of feed rate.",
        "Tolerance" => {
            "Geometric tolerance for path approximation. Smaller = more accurate, slower."
        }
        "Min Cut Radius" | "Min Cutting Radius" => {
            "Blend sharp corners with arcs of at least this radius."
        }
        "Stock Top Z" => "Z height of the stock material top surface.",
        "Scallop Height" => "Target cusp height between passes. 0.05-0.2mm for finishing.",
        "Threshold Angle" => "Angle dividing steep (waterline) from shallow (raster) regions.",
        "Steep Threshold" => "Slope entering the mid-steep scallop band (deg). Below this: raster.",
        "Waterline Threshold" => {
            "Slope entering the very-steep waterline band (deg). Above this: waterline."
        }
        "Raster Stepover" => "Distance between raster passes in the shallow band.",
        "Max Stepdown" => "Maximum Z step between ramp passes.",
        "Z Step" => "Vertical distance between waterline Z levels.",
        "Sampling" => "XY grid resolution for push-cutter sampling.",
        "Bitangency Angle" => {
            "Minimum dihedral angle to detect concave edges. 140-170 deg typical."
        }
        "Min Cut Length" => "Minimum polyline length to include as a pencil pass.",
        "Hookup Distance" => "Max gap between pencil segments to connect into one pass.",
        "Max Depth" => "Maximum V-carve plunge depth. Limits how deep the V-bit goes.",
        "Glue Gap" => "Gap between male/female inlay pieces for glue. 0.05-0.15mm.",
        "Overlap" | "Overlap Distance" => "Overlap between steep and shallow regions.",
        "Wall Clearance" => "Extra clearance from vertical walls.",
        "Max Distance" => "Max XY distance to keep tool down instead of retracting.",
        "Max Angle" => "Maximum ramp angle from horizontal for entry moves.",
        "Min Z" => "Lowest Z the tool will descend to during drop-cutter.",
        "Angle" => "Zigzag/raster angle in degrees. 0 = along X axis.",
        "Fine Stepdown" => "Optional finer Z step for final passes. 0 = disabled.",
        "Stock Offset" => "Extra distance beyond stock boundary to ensure full coverage.",
        "Chamfer Width" => "Width of the chamfer on the face (mm). Depth computed from tool angle.",
        "Tip Offset" => "Distance from V-bit tip to prevent wear. Increases cut depth slightly.",
        "Peck Depth" => "Incremental depth per peck for chip evacuation.",
        "Dwell Time" => "Pause at bottom of drill hole (seconds).",
        "Retract Amt" => "Small retract distance for chip breaking between pecks.",
        "Retract (R)" => {
            "R-plane for this drill cycle: rapid down to here, then feed into material."
        }
        "Angular Step" => "Degrees between radial spokes. Smaller = more passes, finer finish.",
        "Point Spacing" => "Distance between sample points along curves. Smaller = smoother.",
        "Chain Distance" => {
            "Max gap between two projected chains to join with one clearance-height link \
             instead of retracting to safe Z and re-plunging. 0 = off. A cap, not a target: \
             each link is gouge-checked against the surface, kept inside the machining \
             boundary, lifted clear of standing stock, and dropped back to a retract when \
             that clearance reaches safe Z."
        }
        "Angle Threshold" => "Max slope angle (degrees) to consider a surface flat/horizontal.",
        // F3 / D-16.2: one label, shared by every finish op that exposes the
        // dial — so the caveat here is the repo-wide one (a vertical offset,
        // not a surface-normal one), not a per-op note. The "ignored on the
        // shallow band" caveat this dial USED to deserve is gone: since
        // 2026-08-06 all three UnifiedFinish bands honour it.
        "Stock to Leave" => {
            "Finishing allowance kept on the surface for a later pass. Applied as a vertical \
             offset: on a wall sloped at angle A, what remains measured normal to the surface \
             is this value x cos(A)."
        }
        "Slope From" => {
            "Minimum surface slope (degrees) to machine. Faces shallower than this are skipped."
        }
        "Pocket Depth" => "Depth of the inlay pocket measured from stock surface.",
        "Flat Depth" => "Depth for flat-bottom clearing in the inlay pocket. 0 = V-only.",
        "Boundary Offset" => "Offset from the design boundary for the inlay cut. Adjusts fit.",
        "Flat Tool Radius" => "Radius of the flat endmill used to clear the pocket floor.",
        "Spoilboard" => "How far the drill penetrates into the spoilboard below the stock.",
        "Width" => "Width of holding tabs that keep the part attached to stock.",
        "Height" => "Height of holding tabs from the floor of the cut.",
        "Offset Stepover" => "Lateral step between offset cleanup passes around pencil traces.",
        "Pitch" => "Vertical drop per revolution of the helical entry move.",
        "Radius" => "Radius of the helical or arc entry/exit move.",
        "Max Rate" => "Maximum allowable feed rate during optimized sections.",
        "Ramp Rate" => "How quickly feed rate ramps up toward max (mm/min per mm of engagement).",
        "Slope To" => "Maximum surface slope (degrees) to machine. Steeper faces are skipped.",
        "Finishing Passes" => "Spring passes at final depth for dimensional accuracy.",
        "Offset Passes" => "Number of parallel offset passes around pencil traces.",
        "Count" => "Number of holding tabs placed around the profile perimeter.",
        "Continuous" => "Connect passes into a single continuous toolpath without retract.",
        "Direction" => "Cutting direction for this operation.",
        _ => return None,
    })
}

// ── Dressup configuration ────────────────────────────────────────────────

/// Count how many dressup features are currently active.
pub(super) fn dressup_active_count(cfg: &DressupConfig) -> (usize, usize) {
    let total = 8;
    let mut active = 0;
    if !matches!(cfg.entry_style, DressupEntryStyle::None) {
        active += 1;
    }
    if cfg.lead_in_out {
        active += 1;
    }
    if cfg.dogbone {
        active += 1;
    }
    if cfg.arc_fitting {
        active += 1;
    }
    if cfg.link_moves {
        active += 1;
    }
    if cfg.feed_optimization {
        active += 1;
    }
    if cfg.optimize_rapid_order {
        active += 1;
    }
    (active, total)
}

/// Linking tab (W3.2): how moves connect — Entry & Exit and Move
/// Optimization. Split out of the old monolithic dressup panel; the
/// remaining edge-work (arc fitting / dogbone) stays in [`draw_dressup_params`].
pub(super) fn draw_linking_params(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    height_ctx: Option<&HeightContext>,
) {
    // Some dressups are geometrically incompatible with specific operations
    // (project_curve traces many small rings; drop_cutter 3D finish emits
    // hundreds of raster segments — a ramp entry / lead-in / link-move at
    // each one produces diagonal trenches carving through the stock). Grey
    // those controls out so the UI reflects what compute actually does.
    // Read from the SAME registry policy `DressupConfig::normalize_for_op`
    // applies (Phase 1 T5) — no hand-synced duplicate table.
    let dressup_policy = entry.operation.op_type().registry_entry().dressup_policy;
    let op_incompatible_msg: Option<&str> = dressup_policy.strip_all_reason;
    // W1.2/P2-003 — adaptive3d (and other planner-emitted / single-pass
    // ops) coerce the dressup entry style to None via
    // EntryStylePolicy::ForceNone, independently of strip_all_reason. The
    // entry combo was gated only on the latter, so it stayed editable while
    // compute silently ignored it. Gate the entry combo on the entry policy
    // too (lead-in/out and link moves are unaffected by ForceNone, so they
    // keep using op_incompatible_msg).
    let entry_disabled_msg: Option<&str> = op_incompatible_msg.or_else(|| {
        matches!(
            dressup_policy.entry,
            rs_cam_core::compute::catalog::EntryStylePolicy::ForceNone
        )
        .then_some("This operation sets its entry move directly — the dressup entry style isn't used here.")
    });
    let cfg = &mut entry.dressups;
    let section_color = crate::ui::tokens::TEXT_MUTED;

    // ── Entry & Exit ──────────────────────────────────────────
    ui.label(
        egui::RichText::new("Entry & Exit")
            .small()
            .strong()
            .color(section_color),
    );

    ui.horizontal(|ui| {
        ui.label("Entry Style:");
        ui.add_enabled_ui(entry_disabled_msg.is_none(), |ui| {
            let combo = egui::ComboBox::from_id_salt("dressup_entry")
                .selected_text(match cfg.entry_style {
                    DressupEntryStyle::None => "None",
                    DressupEntryStyle::Ramp => "Ramp",
                    DressupEntryStyle::Helix => "Helix",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.entry_style, DressupEntryStyle::None, "None")
                        .on_hover_text("Plunge straight down. Fast but can burn in hard materials.");
                    ui.selectable_value(&mut cfg.entry_style, DressupEntryStyle::Ramp, "Ramp")
                        .on_hover_text("Angled descent into material. Prevents plunge burns. Recommended for most operations.");
                    ui.selectable_value(&mut cfg.entry_style, DressupEntryStyle::Helix, "Helix")
                        .on_hover_text("Spiral descent. Best for deep pockets and hard materials. Spreads heat and load evenly.");
                });
            if let Some(msg) = entry_disabled_msg {
                combo.response.on_hover_text(msg);
            }
        });
    });
    match cfg.entry_style {
        DressupEntryStyle::Ramp => {
            egui::Grid::new("ramp_p")
                .num_columns(2)
                .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
                .min_row_height(crate::ui::tokens::ROW_DENSE)
                .show(ui, |ui| {
                    dv(
                        ui,
                        "  Max Angle:",
                        &mut cfg.ramp_angle,
                        " deg",
                        0.5,
                        0.5..=15.0,
                    );
                });
        }
        DressupEntryStyle::Helix => {
            egui::Grid::new("helix_p")
                .num_columns(2)
                .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
                .min_row_height(crate::ui::tokens::ROW_DENSE)
                .show(ui, |ui| {
                    dv(
                        ui,
                        "  Radius:",
                        &mut cfg.helix_radius,
                        " mm",
                        0.1,
                        0.5..=20.0,
                    );
                    dv(ui, "  Pitch:", &mut cfg.helix_pitch, " mm", 0.1, 0.2..=10.0);
                });
        }
        DressupEntryStyle::None => {}
    }
    {
        let fallback_ctx = HeightContext::simple(10.0, 5.0);
        let ctx = height_ctx.unwrap_or(&fallback_ctx);
        ui.add_space(4.0);
        draw_entry_preview_diagram(ui, cfg, ctx, &entry.heights);
    }

    ui.add_enabled_ui(op_incompatible_msg.is_none(), |ui| {
        let resp = ui
            .checkbox(&mut cfg.lead_in_out, "Lead-in / lead-out")
            .on_hover_text("Add smooth arc transitions at cut start and end. Prevents tool marks at entry/exit points. Best for finishing and profile cuts.");
        if let Some(msg) = op_incompatible_msg {
            resp.on_hover_text(msg);
        }
    });
    if cfg.lead_in_out && op_incompatible_msg.is_none() {
        egui::Grid::new("lead_p")
            .num_columns(2)
            .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
            .show(ui, |ui| {
                dv(
                    ui,
                    "  Radius:",
                    &mut cfg.lead_radius,
                    " mm",
                    0.1,
                    0.5..=20.0,
                );
            });
        draw_lead_in_out_diagram(ui, cfg.lead_radius);
    }

    ui.add_space(6.0);

    // ── Optimization ──────────────────────────────────────────
    ui.label(
        egui::RichText::new("Optimization")
            .small()
            .strong()
            .color(section_color),
    );

    ui.add_enabled_ui(op_incompatible_msg.is_none(), |ui| {
        let resp = ui
            .checkbox(&mut cfg.link_moves, "Link moves (keep tool down)")
            .on_hover_text("Replace short retract-rapid-plunge sequences with slow linear feeds. Major time saver for operations with many small regions. Keeps the tool in the material instead of retracting.");
        if let Some(msg) = op_incompatible_msg {
            resp.on_hover_text(msg);
        }
    });
    if cfg.link_moves && op_incompatible_msg.is_none() {
        egui::Grid::new("link_p")
            .num_columns(2)
            .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
            .show(ui, |ui| {
                dv(
                    ui,
                    "  Max Distance:",
                    &mut cfg.link_max_distance,
                    " mm",
                    0.5,
                    1.0..=50.0,
                );
                dv(
                    ui,
                    "  Feed Rate:",
                    &mut cfg.link_feed_rate,
                    " mm/min",
                    10.0,
                    50.0..=5000.0,
                );
            });
    }

    let feed_opt_reason = crate::state::toolpath::feed_optimization_unavailable_reason(
        &entry.operation,
        entry.stock_source,
    );
    if let Some(reason) = feed_opt_reason {
        // G-LINKFEEDOPT: viewing a tab commits nothing. The disabled
        // checkbox reads a local copy; `dressup_apply` already skips the
        // pass when `feed_opt_stock` is `None`, so the stored flag is inert.
        // Why-disabled lives on hover only (density pass) — the greyed
        // checkbox is the signal; a permanent italic paragraph was noise.
        let mut shown = false;
        ui.add_enabled(
            false,
            egui::Checkbox::new(&mut shown, "Feed rate optimization"),
        )
        .on_disabled_hover_text(reason);
    } else {
        ui.checkbox(&mut cfg.feed_optimization, "Feed rate optimization")
            .on_hover_text("Dynamically adjust feed rate based on stock engagement. Higher feed in light cuts, lower in heavy cuts. Only available for fresh-stock 2D operations.");
    }
    if cfg.feed_optimization && feed_opt_reason.is_none() {
        egui::Grid::new("fopt_p")
            .num_columns(2)
            .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
            .show(ui, |ui| {
                dv(
                    ui,
                    "  Max Rate:",
                    &mut cfg.feed_max_rate,
                    " mm/min",
                    50.0,
                    500.0..=20000.0,
                );
                dv(
                    ui,
                    "  Ramp Rate:",
                    &mut cfg.feed_ramp_rate,
                    " mm/min/mm",
                    10.0,
                    10.0..=2000.0,
                );
            });
    }

    ui.checkbox(&mut cfg.optimize_rapid_order, "Optimize rapid travel order")
        .on_hover_text("Reorder disconnected toolpath segments to minimize total rapid travel distance (TSP heuristic). Pure optimization with no machining risk.");
}

/// Dressup tab (W3.2): edge work / path quality — arc fitting and dogbone
/// overcuts. Entry/exit + optimization live on the Linking tab
/// ([`draw_linking_params`]); the machining boundary lives on Geometry.
pub(super) fn draw_dressup_params(ui: &mut egui::Ui, cfg: &mut DressupConfig) {
    let section_color = crate::ui::tokens::TEXT_MUTED;

    // ── Path Quality ──────────────────────────────────────────
    ui.label(
        egui::RichText::new("Path Quality")
            .small()
            .strong()
            .color(section_color),
    );

    ui.checkbox(&mut cfg.arc_fitting, "Arc fitting (G2/G3)")
        .on_hover_text("Convert sequences of linear segments into smooth G2/G3 arcs. Reduces file size, improves surface finish, and produces smoother machine motion. Safe for all operations.");
    if cfg.arc_fitting {
        egui::Grid::new("arc_p")
            .num_columns(2)
            .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
            .show(ui, |ui| {
                dv(
                    ui,
                    "  Tolerance:",
                    &mut cfg.arc_tolerance,
                    " mm",
                    0.01,
                    0.01..=0.5,
                );
            });
    }

    ui.checkbox(&mut cfg.dogbone, "Dogbone overcuts")
        .on_hover_text("Add circular overcuts at inside corners so parts fit together. Essential for joints, inlays, and press-fit assemblies. Not needed for open pockets or 3D surfaces.");
    if cfg.dogbone {
        egui::Grid::new("dog_p")
            .num_columns(2)
            .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
            .show(ui, |ui| {
                dv(
                    ui,
                    "  Max Angle:",
                    &mut cfg.dogbone_angle,
                    " deg",
                    1.0,
                    45.0..=135.0,
                );
            });
        draw_dogbone_diagram(ui, cfg.dogbone_angle);
    }
}
