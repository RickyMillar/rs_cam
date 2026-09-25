//! The Linking and Dressup tabs, and the parameter-grid helpers every
//! operation editor draws its rows with.
//!
//! `draw_linking_params` and `draw_dressup_params` call each other, so they
//! share one file.

use super::feeds_speeds::draw_entry_preview_diagram;
use super::operations::{draw_dogbone_diagram, draw_lead_in_out_diagram};
use super::{operations, pills};
use crate::state::toolpath::{
    ArcFitParams, DogboneParams, DressupConfig, DressupEntryStyle, HeightContext, LeadParams,
    LinkDressupParams, ToolpathEntry,
};
use crate::ui::automation;
use crate::ui::components::UiExt as _;
use crate::ui::components::ValueRow;
use rs_cam_core::compute::catalog::OperationType;

// --- Parameter grid helpers ---

/// One parameter row's identity: which operation owns it, the registry name
/// the help text is keyed on, and the label the operator reads.
///
/// UI-04: the three used to be one string. The label WAS the key, so
/// re-wording it dropped the tooltip in silence. Holding them together
/// means a row states its key beside the words it shows.
#[derive(Clone, Copy)]
pub(super) struct Param {
    pub op: OperationType,
    pub name: &'static str,
    pub label: &'static str,
}

/// Name one parameter row. The short form the editors call.
pub(super) const fn p(op: OperationType, name: &'static str, label: &'static str) -> Param {
    Param { op, name, label }
}

pub(super) fn dv(
    ui: &mut egui::Ui,
    param: Param,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
) {
    // Delegates to the shared `ValueRow` component (the one labelled-input row).
    let out = ValueRow::new(param.label, val, suffix, speed, range)
        .tooltip(help_for(param.op, param.name))
        .show(ui);
    record_stock_to_leave(ui, param.name, &out);
}

/// The help line for one operation parameter, from the core registry.
///
/// UI-04: the key is the registry parameter NAME, not the visible label. The
/// old `tooltip_for` matched 60 arms against
/// `label.trim().trim_end_matches(':')`, so `"Stepover:"` reached its help
/// only because two spellings agreed by hand, and a label re-word dropped the
/// tooltip in silence. `help_for` cannot miss for a name the registry states.
pub(super) fn help_for(op: OperationType, param: &'static str) -> Option<&'static str> {
    op.registry_entry()
        .param_defs
        .iter()
        .find(|d| d.name == param)
        .and_then(|d| d.help)
}

/// The help line for one dressup field, from `DressupConfig::FIELD_DEFS`.
///
/// A dressup dial is not an operation parameter, so it reads the table CMP-17
/// made the one place the dressup vocabulary is written down.
pub(super) fn dressup_help(field: &'static str) -> Option<&'static str> {
    rs_cam_core::compute::config::DressupConfig::FIELD_DEFS
        .iter()
        .find(|d| d.name == field)
        .map(|d| d.description)
}

/// [`dv`] for a dressup dial, keyed on the published dressup field name.
pub(super) fn dv_dressup(
    ui: &mut egui::Ui,
    field: &'static str,
    label: &str,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
) {
    ValueRow::new(label, val, suffix, speed, range)
        .tooltip(dressup_help(field))
        .show(ui);
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
///
/// UI-04: the test keyed on the visible label, so re-wording the label broke
/// the hook with nothing failing. It now reads the registry parameter name.
/// Adaptive3d spells its field `stock_to_leave_axial` and every other op
/// spells it `stock_to_leave`; both carry the hook, exactly as the label
/// match did.
fn record_stock_to_leave(
    ui: &mut egui::Ui,
    param: &'static str,
    out: &crate::ui::components::ValueRowOutcome,
) {
    if param.starts_with("stock_to_leave") {
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
    param: Param,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    suggestion: Option<pills::PillSuggestion<'_>>,
) -> bool {
    let mut row = ValueRow::new(param.label, val, suffix, speed, range)
        .tooltip(help_for(param.op, param.name));
    let mut click = None;
    if let Some(pill) = suggestion {
        let (s, c) = pill.into_parts();
        row = row.suggest(s);
        click = Some(c);
    }
    let out = row.show(ui);
    record_stock_to_leave(ui, param.name, &out);
    if out.suggested
        && let Some(c) = &click
    {
        c.record();
    }
    out.suggested
}

// ── Dressup configuration ────────────────────────────────────────────────

/// Count how many dressup features are currently active.
pub(super) fn dressup_active_count(cfg: &DressupConfig) -> (usize, usize) {
    let total = 8;
    let mut active = 0;
    if !matches!(cfg.entry_style, DressupEntryStyle::None) {
        active += 1;
    }
    if cfg.lead_in_out.is_some() {
        active += 1;
    }
    if cfg.dogbone.is_some() {
        active += 1;
    }
    if cfg.arc_fitting.is_some() {
        active += 1;
    }
    if cfg.link_moves.is_some() {
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
    tool: Option<&rs_cam_core::compute::tool_config::ToolConfig>,
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
    // The rapid-order pass reads the op's own capability (Face, 3D Rough
    // refuse it: the planner stock depends on the cut order). Grey the box
    // out so it does not claim an effect compute never applies.
    let reorder_ok = entry
        .operation
        .transform_capabilities()
        .allows_rapid_reorder;
    // G10 Q6 and D2: the helix radius resolves against the tool (the rule
    // 0.3 x D, or the operator value, capped at the flat bottom).
    let cutter = tool.map(rs_cam_core::compute::cutter::build_cutter);
    let cfg = &mut entry.dressups;

    // ── Entry & Exit ──────────────────────────────────────────
    crate::ui::components::SectionHeader::new("Entry & Exit").show(ui);

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
            ui.param_grid("ramp_p", |ui| {
                dv_dressup(
                    ui,
                    "ramp_angle",
                    "  Max Angle:",
                    &mut cfg.ramp_angle,
                    " deg",
                    0.5,
                    0.5..=15.0,
                );
                dv_dressup(
                    ui,
                    "entry_clearance_mm",
                    "  Clearance:",
                    &mut cfg.entry_clearance_mm,
                    " mm",
                    0.1,
                    0.0..=5.0,
                );
            });
        }
        DressupEntryStyle::Helix => {
            // G10 D2: `None` is the rule 0.3 x D; `Some` is an operator
            // value. Unticking the rule starts the value at the radius the
            // rule gives this tool.
            let resolved = cutter.as_ref().map(|c| cfg.helix_radius_for(c));
            let radius_range = 0.5..=20.0;
            let mut rule = cfg.helix_radius.is_none();
            let resp = ui
                .checkbox(
                    &mut rule,
                    format!(
                        "Helix radius {} x D (rule)",
                        rs_cam_core::compute::config::HELIX_RADIUS_OVER_D
                    ),
                )
                .on_hover_text(
                    "Repo rule, no source (G10): the helix radius is 0.3 x the tool \
                     diameter. Untick to set an operator value. Either way the engine caps \
                     the radius at the flat bottom, so the helix leaves no core.",
                );
            if resp.changed() {
                cfg.helix_radius = if rule {
                    None
                } else {
                    Some(resolved.map_or(*radius_range.start(), |h| h.requested_mm))
                };
            }
            if let Some(h) = resolved {
                let text = if h.capped() {
                    format!(
                        "  Emits r {:.2} mm: capped {:.2} → {:.2} mm (no-core rule, geometry)",
                        h.emitted_mm, h.requested_mm, h.emitted_mm
                    )
                } else {
                    format!("  Emits r {:.2} mm", h.emitted_mm)
                };
                ui.label(
                    egui::RichText::new(text)
                        .small()
                        .color(crate::ui::tokens::TEXT_MUTED),
                );
            }
            ui.param_grid("helix_p", |ui| {
                if let Some(radius) = cfg.helix_radius.as_mut() {
                    dv_dressup(
                        ui,
                        "helix_radius",
                        "  Radius:",
                        radius,
                        " mm",
                        0.1,
                        radius_range,
                    );
                }
                dv_dressup(
                    ui,
                    "helix_pitch",
                    "  Pitch:",
                    &mut cfg.helix_pitch,
                    " mm",
                    0.1,
                    0.2..=10.0,
                );
                dv_dressup(
                    ui,
                    "entry_clearance_mm",
                    "  Clearance:",
                    &mut cfg.entry_clearance_mm,
                    " mm",
                    0.1,
                    0.0..=5.0,
                );
            });
        }
        DressupEntryStyle::None => {}
    }
    {
        let fallback_ctx = HeightContext::simple(10.0, 5.0);
        let ctx = height_ctx.unwrap_or(&fallback_ctx);
        ui.add_space(4.0);
        let helix_radius_mm = cutter.as_ref().map(|c| cfg.helix_radius_for(c).emitted_mm);
        draw_entry_preview_diagram(ui, cfg, helix_radius_mm, ctx, &entry.heights);
    }

    ui.add_enabled_ui(op_incompatible_msg.is_none(), |ui| {
        // CUT-13: the parameters live inside the `Option`. The checkbox reads
        // a local bool and puts the module defaults back on a fresh enable.
        let mut on = cfg.lead_in_out.is_some();
        let resp = ui
            .checkbox(&mut on, "Lead-in / lead-out")
            .on_hover_text("Add smooth arc transitions at cut start and end. Prevents tool marks at entry/exit points. Best for finishing and profile cuts.");
        if resp.changed() {
            cfg.lead_in_out = on.then(LeadParams::default);
        }
        if let Some(msg) = op_incompatible_msg {
            resp.on_hover_text(msg);
        }
    });
    if op_incompatible_msg.is_none()
        && let Some(lead) = cfg.lead_in_out.as_mut()
    {
        ui.param_grid("lead_p", |ui| {
            dv_dressup(
                ui,
                "lead_radius",
                "  Radius:",
                &mut lead.radius,
                " mm",
                0.1,
                0.5..=20.0,
            );
        });
        let radius = lead.radius;
        draw_lead_in_out_diagram(ui, radius);
    }

    ui.add_space(6.0);

    // ── Optimization ──────────────────────────────────────────
    crate::ui::components::SectionHeader::new("Optimization").show(ui);

    ui.add_enabled_ui(op_incompatible_msg.is_none(), |ui| {
        let mut on = cfg.link_moves.is_some();
        let resp = ui
            .checkbox(&mut on, "Link moves (keep tool down)")
            .on_hover_text("Replace short retract-rapid-plunge sequences with slow linear feeds. Major time saver for operations with many small regions. Keeps the tool in the material instead of retracting.");
        if resp.changed() {
            cfg.link_moves = on.then(LinkDressupParams::default);
        }
        if let Some(msg) = op_incompatible_msg {
            resp.on_hover_text(msg);
        }
    });
    if op_incompatible_msg.is_none()
        && let Some(link) = cfg.link_moves.as_mut()
    {
        ui.param_grid("link_p", |ui| {
            dv_dressup(
                ui,
                "link_max_distance",
                "  Max Distance:",
                &mut link.max_distance,
                " mm",
                0.5,
                1.0..=50.0,
            );
            dv_dressup(
                ui,
                "link_feed_rate",
                "  Feed Rate:",
                &mut link.feed_rate,
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
        ui.param_grid("fopt_p", |ui| {
            dv_dressup(
                ui,
                "feed_max_rate",
                "  Max Rate:",
                &mut cfg.feed_max_rate,
                " mm/min",
                50.0,
                500.0..=20000.0,
            );
            dv_dressup(
                ui,
                "feed_ramp_rate",
                "  Ramp Rate:",
                &mut cfg.feed_ramp_rate,
                " mm/min/mm",
                10.0,
                10.0..=2000.0,
            );
        });
    }

    ui.add_enabled_ui(reorder_ok, |ui| {
        let resp = ui
            .checkbox(&mut cfg.optimize_rapid_order, "Optimize rapid travel order")
            .on_hover_text("Reorder disconnected toolpath segments to minimize total rapid travel distance (TSP heuristic). Only operations whose cut order does not change what later moves meet allow it.");
        if !reorder_ok {
            resp.on_disabled_hover_text("This operation keeps its planned cut order: its entries are planned against the stock that earlier cuts leave, so a reorder would plunge into uncut stock.");
        }
    });
}

/// Dressup tab (W3.2): edge work / path quality — arc fitting and dogbone
/// overcuts. Entry/exit + optimization live on the Linking tab
/// ([`draw_linking_params`]); the machining boundary lives on Geometry.
pub(super) fn draw_dressup_params(ui: &mut egui::Ui, cfg: &mut DressupConfig) {
    // ── Path Quality ──────────────────────────────────────────
    crate::ui::components::SectionHeader::new("Path Quality").show(ui);

    let mut arc_on = cfg.arc_fitting.is_some();
    if ui
        .checkbox(&mut arc_on, "Arc fitting (G2/G3)")
        .on_hover_text("Convert sequences of linear segments into smooth G2/G3 arcs. Reduces file size, improves surface finish, and produces smoother machine motion. Safe for all operations.")
        .changed()
    {
        cfg.arc_fitting = arc_on.then(ArcFitParams::default);
    }
    if let Some(arc) = cfg.arc_fitting.as_mut() {
        ui.param_grid("arc_p", |ui| {
            dv_dressup(
                ui,
                "arc_tolerance",
                "  Tolerance:",
                &mut arc.tolerance,
                " mm",
                0.01,
                0.01..=0.5,
            );
        });
    }

    let mut dogbone_on = cfg.dogbone.is_some();
    if ui
        .checkbox(&mut dogbone_on, "Dogbone overcuts")
        .on_hover_text("Add circular overcuts at inside corners so parts fit together. Essential for joints, inlays, and press-fit assemblies. Not needed for open pockets or 3D surfaces.")
        .changed()
    {
        cfg.dogbone = dogbone_on.then(DogboneParams::default);
    }
    if let Some(dogbone) = cfg.dogbone.as_mut() {
        ui.param_grid("dog_p", |ui| {
            dv_dressup(
                ui,
                "dogbone_angle",
                "  Max Angle:",
                &mut dogbone.angle,
                " deg",
                1.0,
                45.0..=135.0,
            );
        });
        let angle = dogbone.angle;
        draw_dogbone_diagram(ui, angle);
    }
}
