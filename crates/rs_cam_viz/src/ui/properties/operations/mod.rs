mod bore;
mod boundary_2d;
mod drill;
mod engrave;
mod finishing;
mod height_diagram;
mod project;
pub mod registry;
mod shape_diagrams;
mod surface_3d;
mod validate;

// Re-export all draw_*_params functions so callers don't need to change imports.
pub(super) use bore::draw_bore_params;
pub(super) use boundary_2d::{
    draw_adaptive_params, draw_face_params, draw_inlay_params, draw_inlay_plug_params,
    draw_pocket_params, draw_profile_params, draw_vcarve_params, draw_zigzag_params,
};
pub(super) use drill::{cycle_draws_peck, draw_alignment_pin_drill_params, draw_drill_params};
pub(super) use engrave::{draw_chamfer_params, draw_trace_params};
pub(super) use finishing::{
    draw_horizontal_finish_params, draw_radial_finish_params, draw_ramp_finish_params,
    draw_spiral_finish_params,
};
pub(super) use project::draw_project_curve_params;
pub(super) use surface_3d::{
    draw_adaptive3d_params, draw_dropcutter_params, draw_pencil_params, draw_scallop_params,
    draw_steep_shallow_params, draw_unified_finish_params, draw_waterline_params,
};

// The P4 split moved three topics into children beside this file. Each name
// keeps its module path through these re-exports, so `ui/properties/mod.rs`
// and the sibling operation editors import exactly what they imported before.
pub use height_diagram::{
    DiagramFloor, FaceFloor, FloorInputs, FloorRun, admit_heights_edit, draw_height_diagram,
    floor_runs, pick_line, round_pin,
};
pub(super) use shape_diagrams::{
    StepoverPattern, draw_dogbone_diagram, draw_hole_diagram, draw_inlay_diagram,
    draw_lead_in_out_diagram, draw_outline_diagram, draw_pencil_diagram, draw_radial_diagram,
    draw_ramp_finish_diagram, draw_spiral_diagram, draw_steep_shallow_diagram,
    draw_stepover_diagram, draw_tab_diagram,
};
pub use validate::{
    DepthBeyondStock, ThroughCut, ToolpathValidationContext, collect_diagnostics,
    depth_beyond_stock, profile_through_cut, profile_through_cut_line, validate_toolpath,
    validate_toolpath_config,
};
pub(in crate::ui::properties) use validate::{hole_refusal, rest_consumer_refusal};

use crate::state::toolpath::{
    HeightContext, HeightMode, HeightReference, HeightsConfig, OperationType, ReferenceOffset,
};
use crate::ui::components::{NumberSpec, PropRow, Role, number_field, optional_number_field};
use crate::ui::properties::placement;
use rs_cam_core::compute::floor::{Floor, FloorRefusal};

// Feed / plunge editing moved off the per-op Geometry panel into the
// Feeds & Speeds tab's SPEED section (W3.2) — the `draw_feed_params` helper
// (and its `FeedsResult` / `dv_pill` deps) was retired with the recharter.
//
// The per-operation spindle RPM override moved to the Feeds-card SPEED section
// (W3.1) as a `PrecedenceField`, where the project default RPM is reachable and
// the override-vs-default precedence renders honestly. The old in-place
// `draw_spindle_rpm_row` (hardcoded 18 000 default) was retired.

// ── Heights panel ────────────────────────────────────────────────────────

/// How one height row is *rendered*: every mode is shown as an offset from a
/// named reference, including `Auto` and `Manual`.
///
/// This projection is a **pure read** — the signature takes `&HeightMode` so it
/// cannot write, and that is the whole point.
///
/// G-HEIGHTSTAB (2026-08-23): the row used to promote `Auto` →
/// `FromReference(default)` and `Manual` → `FromReference(nearest)` at the top
/// of its *draw* function, so merely rendering the Heights tab rewrote the
/// stored config. The panel's write-back (`write_entry_config_to_session`) then
/// saw `heights` change, stamped `stale_since`, and auto-regeneration took the
/// op apart. On any operation whose `default_depth_for_heights()` is 0 — the
/// entire 3D family, which reports `DepthSemantics::None` — the promoted bottom
/// row became `FromReference { StockTop, -0.0 }`: bottom == stock top, and the
/// promotion also flipped `ResolvedHeights::bottom_pinned` from false to true,
/// so the operation honoured a floor at the stock top and regenerated to **zero
/// moves**. Opening a read-only tab destroyed a healthy 7 672-move toolpath.
///
/// Nothing here may write to `mode`; see [`commit_height_row`] for the one
/// place that does, and what gates it.
fn height_row_display(
    mode: &HeightMode,
    default_ref: HeightReference,
    default_z: f64,
    ctx: &HeightContext,
) -> ReferenceOffset {
    match *mode {
        HeightMode::FromReference(ref_offset) => ref_offset,
        // `default_z` is the absolute Z the CORE resolver produces for this
        // row, so the displayed value can never disagree with what generation
        // actually does.
        HeightMode::Auto => ReferenceOffset {
            reference: default_ref,
            offset: default_z - default_ref.resolve_z(ctx),
        },
        HeightMode::Manual(abs_z) => {
            let best_ref = find_nearest_reference(abs_z, ctx);
            ReferenceOffset {
                reference: best_ref,
                offset: abs_z - best_ref.resolve_z(ctx),
            }
        }
    }
}

/// Write an edited row back to the stored mode.
///
/// `edited` must be true only when the user moved a widget **this frame**
/// (a DragValue change or a reference pick). Rendering alone never sets it, so
/// viewing the Heights tab leaves `Auto` as `Auto` — which is what keeps
/// `bottom_pinned` false and the operation's own depth semantics in charge.
fn commit_height_row(mode: &mut HeightMode, display: ReferenceOffset, edited: bool) {
    if edited {
        *mode = HeightMode::FromReference(display);
    }
}

/// The findings of the toolpath by field key (`Placement::by_field`).
pub(in crate::ui::properties) use super::placement::FieldFindings;

/// The state of one field row of the Heights tab: the role of the outline
/// and the problem detail on the hover (W3d).
#[derive(Debug, Default, Clone, PartialEq)]
struct RowState {
    role: Option<Role>,
    detail: Option<String>,
}

impl RowState {
    /// The state from the findings that `placement::place` anchors to `key`.
    fn of(findings: &FieldFindings<'_>, key: &str) -> Self {
        placement::note_drawn(key);
        let list = findings.get(key).map_or(&[][..], Vec::as_slice);
        Self {
            role: placement::tool_role(list),
            detail: placement::tool_hover(list),
        }
    }

    /// Raise the state to `role` when `role` ranks higher, and put `detail`
    /// first on the hover.
    fn raise(&mut self, role: Role, detail: &str) {
        if self
            .role
            .is_none_or(|old| role.severity_rank() < old.severity_rank())
        {
            self.role = Some(role);
        }
        self.detail = Some(match self.detail.take() {
            Some(old) => format!("{detail}\n\n{old}"),
            None => detail.to_owned(),
        });
    }
}

/// One row of the heights grid.
struct HeightRow<'a> {
    /// The label, in sentence case.
    label: &'a str,
    /// The field key (`diagnostics::presentation::HEIGHT_KEYS`).
    key: &'static str,
    /// The explanation on the hover.
    tooltip: &'a str,
    /// The absolute Z that the core resolver gives the row (see
    /// [`height_row_display`]).
    default_z: f64,
    /// The text of the reference combo while the row is `Auto`.
    auto_label: &'a str,
}

/// The width of the value field of a height row and of the Bottom height
/// row. The unit follows it; the reference combo takes the rest of the
/// control.
const HEIGHT_VALUE_WIDTH: f32 = 56.0;

/// The size of the value field of a height row in the control area `ui`.
fn height_value_size(ui: &egui::Ui) -> egui::Vec2 {
    egui::vec2(
        HEIGHT_VALUE_WIDTH.min(ui.available_width()),
        ui.available_height(),
    )
}

/// The hover of the "Auto" entry of a reference combo.
const AUTO_HINT: &str = "The resolver sets this height from the stock, the model and the \
     operation. It follows them when they change.";

/// A height row (W3d): the absolute Z with the unit "mm" and the reference
/// combo ("Auto", "Stock top", ...) in one `PropRow`.
///
/// F5: the row shows the absolute Z. The diagram, the Linking diagram and
/// MCP `heights` show the same number. The reference and the offset are on
/// the hover. An edit of the value keeps the reference of the row: it
/// writes `FromReference { reference, offset: z - reference Z }`. A pick of
/// a reference keeps the Z (ruling L-10, as the floor row does). A pick of
/// "Auto" gives the row back to the resolver.
///
/// R5 (2026-09-18): a pinned row names its reference in the combo, and an
/// `Auto` row reads "Auto", so a diagram drag that pins a height shows on
/// the row (`planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §4.2).
///
/// G-HEIGHTSTAB: a draw writes nothing. Only a changed value or a clicked
/// entry writes `mode`.
fn draw_height_row(
    ui: &mut egui::Ui,
    row: &HeightRow<'_>,
    mode: &mut HeightMode,
    ctx: &HeightContext,
    state: &RowState,
) {
    let display = height_row_display(mode, HeightReference::StockTop, row.default_z, ctx);
    let ref_z = display.reference.resolve_z(ctx);
    let mut z = ref_z + display.offset;
    let auto = mode.is_auto();
    let place = format!(
        "{:.2} mm {}",
        display.offset.abs(),
        ref_label(display.reference, display.offset)
    );
    let source = if auto {
        format!("Auto: the resolver sets this height ({place}).")
    } else {
        format!("Pinned: {place}. {PINNED_HINT}")
    };
    let hover = format!("{}\n\n{source}", row.tooltip);
    let selected = if auto {
        row.auto_label
    } else {
        display.reference.label()
    };
    let mut picked: Option<HeightMode> = None;
    let out = PropRow::new(row.label)
        .state(state.role)
        .hover(Some(&hover))
        .detail(state.detail.as_deref())
        .show(ui, |ui| {
            // W3f: the unit sits in the well after the value, then the
            // reference combo.
            let value = number_field(
                ui,
                &mut z,
                &NumberSpec::new(0.5)
                    .range(ref_z - 500.0..=ref_z + 500.0)
                    .decimals(2)
                    .unit("mm"),
                height_value_size(ui),
            );
            let combo = egui::ComboBox::from_id_salt(("height_ref", row.key))
                .truncate()
                .width(ui.available_width())
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    if ui
                        .add(egui::Button::selectable(auto, "Auto"))
                        .on_hover_text(AUTO_HINT)
                        .clicked()
                    {
                        picked = Some(HeightMode::Auto);
                    }
                    for &href in HeightReference::ALL {
                        let href_z = href.resolve_z(ctx);
                        // `.clicked()`: a pick of the reference that an
                        // Auto row only displays is still an explicit pin.
                        if ui
                            .add(egui::Button::selectable(
                                !auto && display.reference == href,
                                href.label(),
                            ))
                            .on_hover_text(format!(
                                "{} is at Z {href_z:.2}. The height keeps its Z.",
                                href.label()
                            ))
                            .clicked()
                        {
                            picked = Some(HeightMode::FromReference(ReferenceOffset {
                                reference: href,
                                offset: z - href_z,
                            }));
                        }
                    }
                });
            (value, combo.response)
        });
    let (value, combo) = out.inner;
    let edited = value.changed();
    let _ = value.on_hover_text(&out.hover);
    let _ = combo.on_hover_text(&out.hover);
    commit_height_row(
        mode,
        ReferenceOffset {
            reference: display.reference,
            offset: z - ref_z,
        },
        edited,
    );
    if let Some(next) = picked {
        *mode = next;
    }
}

/// The hover of a pinned height row.
const PINNED_HINT: &str = concat!(
    "The resolver does not move this height when the stock, ",
    "the model or the operation changes. Pick Auto to release it."
);

/// Descriptive label for the reference dropdown: "above/below Stock top" etc.
fn ref_label(reference: HeightReference, offset: f64) -> String {
    let dir = if offset >= 0.0 { "above" } else { "below" };
    format!("{dir} {}", reference.label())
}

/// Find the nearest reference point to an absolute Z value.
fn find_nearest_reference(z: f64, ctx: &HeightContext) -> HeightReference {
    let mut best = HeightReference::StockTop;
    let mut best_dist = f64::INFINITY;
    for &href in HeightReference::ALL {
        let ref_z = href.resolve_z(ctx);
        let dist = (z - ref_z).abs();
        if dist < best_dist {
            best_dist = dist;
            best = href;
        }
    }
    best
}

/// F1.19 / G-BOTTOMPIN: the sentence the Heights tab prints beside the Bottom
/// row when the operation does not read a pinned Bottom Z.
///
/// `None` means the pin DOES drive the cut, so the row is offered plain.
/// `Some(note)` names the dial that really sets the floor.
///
/// The core half measured the fact and declared it as
/// [`rs_cam_core::compute::catalog::Capabilities::floor_owner`]: three of
/// the twenty-four operations read `heights.bottom_z` (`HeightsBottom`). On
/// Pocket, Profile, Zigzag and Adaptive 2D (`OpFloor`, P3.2) the session
/// refuses a pinned bottom, and on the other seventeen (`OpDial`) a pinned
/// bottom reaches no emitted motion. This match
/// carries its own arm per operation because each family names a DIFFERENT
/// dial, and the sentry
/// (`crates/rs_cam_viz/tests/bottom_z_pin_note_g_bottompin.rs`) ties the two
/// enumerations together so they cannot drift.
///
/// The panel ANNOTATES rather than disables. A disabled row cannot be set
/// back to Auto, and a legacy project can carry a stale pin. The operator
/// must keep the one control that clears it. F033: the order rule
/// (`compute::config::height_order`) does not check the bottom of an op that
/// does not read it, so an unused pin raises no badge and refuses nothing.
pub fn bottom_z_pin_note(op_type: OperationType) -> Option<&'static str> {
    use crate::state::toolpath::OperationType as Op;

    const DEPTH: &str = "Not used by this operation. Its Depth field sets the floor.";
    const FLOOR: &str = "The Bottom height row sets the floor of this operation. The door \
         refuses a pinned Heights bottom.";
    const MAX_DEPTH: &str = "Not used by this operation. Its Max Depth sets the floor.";
    const POCKET_DEPTH: &str = "Not used by this operation. Its Pocket Depth sets the floor.";
    const PLUG_DEPTH: &str =
        "Not used by this operation. Its Pocket Depth, Glue Gap and Proud Height set the floor.";
    const WIDTH: &str = "Not used by this operation. Its Chamfer Width sets the floor.";
    const PIN_DRILL: &str = "Not used by this operation. Its Depth and Spoilboard set the floor.";
    const CURVE: &str = "Not used by this operation. The projected surface sets the floor.";
    const SURFACE: &str = "Not used by this operation. The model surface sets the floor.";

    // One arm per operation. The `match` is exhaustive, so a new operation
    // cannot be added to the catalog without an answer here, and the sentry
    // ties every answer to `capabilities().floor_owner`.
    match op_type {
        // The three that read `heights.bottom_z`. No note.
        Op::Adaptive3d | Op::UnifiedFinish | Op::Waterline => None,
        Op::Face => Some(DEPTH),
        Op::Pocket => Some(FLOOR),
        Op::Profile => Some(FLOOR),
        Op::Adaptive => Some(FLOOR),
        Op::Zigzag => Some(FLOOR),
        Op::Trace => Some(DEPTH),
        Op::Drill => Some(DEPTH),
        Op::Bore => Some(FLOOR),
        Op::VCarve => Some(MAX_DEPTH),
        Op::Inlay => Some(POCKET_DEPTH),
        Op::InlayPlug => Some(PLUG_DEPTH),
        Op::Chamfer => Some(WIDTH),
        Op::AlignmentPinDrill => Some(PIN_DRILL),
        Op::ProjectCurve => Some(CURVE),
        Op::DropCutter => Some(SURFACE),
        Op::Scallop => Some(SURFACE),
        Op::Pencil => Some(SURFACE),
        Op::HorizontalFinish => Some(SURFACE),
        Op::SteepShallow => Some(SURFACE),
        Op::RampFinish => Some(SURFACE),
        Op::SpiralFinish => Some(SURFACE),
        Op::RadialFinish => Some(SURFACE),
    }
}

/// The Top row tooltip for the source of the resolved Top (P3.1).
const fn top_tooltip(source: rs_cam_core::session::TopSource) -> &'static str {
    match source {
        rs_cam_core::session::TopSource::AutoFaces => {
            "Top of material. Cutting starts at this Z. Auto: the highest picked face. On \
             an op with a Bottom height row (a floor), and on Face and Trace, the stock top \
             when it is higher; the depth still measures from the face."
        }
        rs_cam_core::session::TopSource::AutoHoleRims => {
            "Top of material. Cutting starts at this Z. Auto: the highest hole rim, or the \
             stock top when it is higher. Each hole measures its depth from its own rim."
        }
        rs_cam_core::session::TopSource::AutoStockTop | rs_cam_core::session::TopSource::Pinned => {
            "Top of material. Cutting starts at this Z. Usually the stock top surface."
        }
    }
}

/// The Bottom height row tooltip for an operation that DOES read the pin.
const BOTTOM_Z_TOOLTIP: &str = "Deepest cut depth. The tool stops at this Z.";

/// The label of the Top height row (ruling Q8: Fusion "Top height").
pub(crate) const TOP_HEIGHT_LABEL: &str = "Top height";

/// The label of the Bottom height row (D6, ruling Q8: Fusion "Bottom
/// height", FreeCAD "Final Depth"). On an `OpFloor` op the row sets the
/// floor of the op.
pub(crate) const BOTTOM_HEIGHT_LABEL: &str = "Bottom height";

/// The reference combo text of an `Auto` Top height row: the source of the
/// resolved Top (P3.1, UX-1), so a face pick shows on the row.
const fn top_auto_label(source: rs_cam_core::session::TopSource) -> &'static str {
    match source {
        rs_cam_core::session::TopSource::AutoFaces => "Auto: faces",
        rs_cam_core::session::TopSource::AutoHoleRims => "Auto: hole rims",
        rs_cam_core::session::TopSource::AutoStockTop | rs_cam_core::session::TopSource::Pinned => {
            "Auto"
        }
    }
}

/// The floor of an `OpFloor` op on the Heights tab (D6): the draft floor
/// that the Bottom height row edits, its references and the caution of a
/// floor below the stock bottom.
pub(in crate::ui::properties) struct BottomHeight<'a> {
    /// The draft floor. The caller writes it back to the config.
    pub floor: &'a mut Floor,
    /// The floor references; `None` when the heights do not resolve.
    pub inputs: Option<&'a FloorInputs>,
    /// G-DEPTHSTOCK: the floor below the stock bottom, or `None`.
    pub depth_caution: Option<&'a DepthBeyondStock>,
}

/// The Heights tab rows (W3d): ONE column of `PropRow`s. Clearance,
/// Retract, Feed, Top height, Bottom height.
///
/// D6 (REFERENCE_AUDIT row D6, Fusion "Bottom height", FreeCAD "Final
/// Depth"): on an op that holds a floor (`FloorOwner::OpFloor`) the row
/// under Top height is the Bottom height row of the floor
/// ([`draw_bottom_height`]). It is the one editor of the floor (IC1); the
/// Passes tab has no floor row. The other ops keep the Bottom height row of
/// the Heights config. When the heights do not resolve, only the Bottom
/// height row is drawn, so the floor stays editable.
///
/// A problem sits on its row (design rule 3): a finding that
/// `placement::place` anchors to the key of a row, the last refused edit
/// (F033) and a model reference with no mesh (L-4) are the state of the row
/// and the detail on its hover. The notes that stood under the grid are
/// the hovers of their rows.
pub(super) fn draw_heights_params(
    ui: &mut egui::Ui,
    heights: &mut HeightsConfig,
    answer: Option<&rs_cam_core::session::ToolpathHeights>,
    op_type: OperationType,
    bottom: Option<BottomHeight<'_>>,
    findings: &FieldFindings<'_>,
    refusal: &mut crate::state::panels::HeightsRefusal,
) {
    // F033: the last refused edit is the Danger state of the row it edited.
    let now = ui.input(|i| i.time);
    let refused = refusal
        .field(now)
        .zip(refusal.text(now))
        .map(|(key, text)| (key, text.to_owned()));
    let state_of = |key: &'static str| {
        let mut state = RowState::of(findings, key);
        if let Some((refused_key, text)) = &refused
            && *refused_key == key
        {
            state.raise(Role::Danger, text);
        }
        state
    };
    // Without resolved heights no height-order finding exists (the order
    // rule reads `answer.resolve`), so only the floor row is drawn.
    let Some(answer) = answer else {
        if let Some(bottom) = bottom {
            draw_bottom_height(ui, op_type, bottom, state_of("floor"));
        }
        return;
    };
    let ctx = &answer.context;
    // What an `Auto` row shows is the value the CORE resolver produces for
    // THIS config, not a second set of defaults kept here. P3.1: THE
    // session rule, so an auto Top shows the face top that generation cuts
    // from.
    let auto = answer.resolve(heights);
    let top_source = answer.top_source(heights);
    // L-4: a model reference on a model with no mesh refuses generation. It
    // is the Danger state of the row that reads the model.
    let model_refusal = answer.model_reference_refusal(heights);
    let reads_model = |mode: &HeightMode| {
        matches!(
            mode,
            HeightMode::FromReference(ReferenceOffset {
                reference: HeightReference::ModelTop | HeightReference::ModelBottom,
                ..
            })
        )
    };

    // F1.19 / G-BOTTOMPIN: the Bottom row's tooltip used to say that
    // pinning the row overrides the operation's floor. That is true on three
    // operations and false on twenty-one. The note replaces the claim on the
    // twenty-one and names the dial that does set the floor. The row stays
    // editable, so an operator can still read a stored pin and set it back
    // to Auto.
    let pin_note = bottom_z_pin_note(op_type);
    let bottom_tooltip = match pin_note {
        Some(note) => format!("{note} The row still shows the stored value."),
        None => BOTTOM_Z_TOOLTIP.to_owned(),
    };
    let top_tip = format!("{} Now: {}.", top_tooltip(top_source), top_source.label());

    // F033: the rows edit a draft. The one order gate
    // (`admit_heights_edit`) writes it after the rows, so a value that
    // would invert the stack is refused before it reaches the config.
    let mut draft = heights.clone();
    let op_floor = bottom.is_some();
    let mut row = |row: HeightRow<'_>, mode: &mut HeightMode| {
        let mut state = state_of(row.key);
        if let Some(text) = &model_refusal
            && reads_model(mode)
        {
            state.raise(Role::Danger, text);
        }
        draw_height_row(ui, &row, mode, ctx, &state);
    };
    row(
        HeightRow {
            label: "Clearance",
            key: "clearance_z",
            tooltip: "Highest safe height. Rapid moves between separate operations travel at \
                      this Z.",
            default_z: auto.clearance_z,
            auto_label: "Auto",
        },
        &mut draft.clearance_z,
    );
    row(
        HeightRow {
            label: "Retract",
            key: "retract_z",
            tooltip: "Rapid travel height within an operation. The tool retracts here \
                      between cutting passes.",
            default_z: auto.retract_z,
            auto_label: "Auto",
        },
        &mut draft.retract_z,
    );
    row(
        HeightRow {
            label: "Feed",
            key: "feed_z",
            tooltip: "Approach height. The tool changes from rapid to feed rate here before \
                      it plunges into the material.",
            default_z: auto.feed_z,
            auto_label: "Auto",
        },
        &mut draft.feed_z,
    );
    row(
        HeightRow {
            label: TOP_HEIGHT_LABEL,
            key: "top_z",
            tooltip: &top_tip,
            default_z: auto.top_z,
            auto_label: top_auto_label(top_source),
        },
        &mut draft.top_z,
    );
    // D6: an `OpFloor` op draws the Bottom height row of its floor here.
    if !op_floor {
        row(
            HeightRow {
                label: BOTTOM_HEIGHT_LABEL,
                key: "bottom_z",
                tooltip: &bottom_tooltip,
                default_z: auto.bottom_z,
                auto_label: "Auto",
            },
            &mut draft.bottom_z,
        );
    }
    if let Some(bottom) = bottom {
        // The floor row is the bottom of an `OpFloor` op, so a finding
        // anchored to `bottom_z` is its state too; no other row draws it.
        let mut state = state_of("floor");
        let bottom_z = state_of("bottom_z");
        if let Some(role) = bottom_z.role {
            state.raise(role, bottom_z.detail.as_deref().unwrap_or_default());
        }
        draw_bottom_height(ui, op_type, bottom, state);
    }
    // L-4: a model refusal that no row reads (the Bottom of an `OpFloor`
    // op, which no row here draws) keeps one line.
    if let Some(text) = &model_refusal {
        let on_a_row = [
            &draft.clearance_z,
            &draft.retract_z,
            &draft.feed_z,
            &draft.top_z,
        ]
        .into_iter()
        .chain((!op_floor).then_some(&draft.bottom_z))
        .any(reads_model);
        if !on_a_row {
            ui.add(crate::ui::components::OneLine::new(text.as_str()));
        }
    }

    let _ = admit_heights_edit(ui, heights, draft, answer, op_type, refusal);
}

/// The text of the closed reference combo of the Bottom height row (W3g):
/// core `Floor::reference_label`, with a short form for the two "each"
/// references, so the text fits the combo beside the value at 340 points.
/// The open list and the hover keep the full label.
fn floor_display_label(floor: &Floor) -> &'static str {
    match floor {
        Floor::EachFace { .. } => "Each face",
        Floor::EachHole { .. } => "Each hole",
        _ => floor.reference_label(),
    }
}

/// The hover of the reference combo of the Bottom height row (ruling L-10).
const FLOOR_FROM_HOVER: &str = "The floor of the operation: the tool cuts down to this Z. The \
    combo sets the reference. A change of reference keeps the floor Z: the value is \
    expressed again from the new reference.";

/// The short reason a reference is not offered, for the hover of its
/// disabled entry.
fn floor_reason(refusal: FloorRefusal) -> &'static str {
    match refusal {
        FloorRefusal::NoModelMesh => "The model has no mesh, so it has no Z extent.",
        FloorRefusal::NoFace => "No up face is picked in the selection.",
        FloorRefusal::OutOfDomain { .. } | FloorRefusal::AboveTop { .. } => {
            "The floor would be at or above the Top height."
        }
        FloorRefusal::NotFinite => "The floor is not a finite number.",
        FloorRefusal::FaceLost => "The floor face is not on the model now.",
        FloorRefusal::NotAFloorItem { .. } => "The floor item is not a face.",
        FloorRefusal::NoHoleDepth => "No hole of the selection has a known depth.",
        FloorRefusal::NotForThisOp { .. } => "This operation cannot cut to this floor.",
        FloorRefusal::Unset => "The floor is not set.",
    }
}

/// The resolved floor of the Bottom height row, for its hover: `Z ..`, the
/// range of the item floors (FR-2 F25), or why the floor has no Z.
fn resolved_floor_text(floor: &Floor, inputs: Option<&FloorInputs>) -> String {
    let refs = inputs.map(|inputs| inputs.refs);
    // F25: each face, each hole, or a depth from the rim of each hole.
    let per_item = matches!(floor, Floor::EachFace { .. } | Floor::EachHole { .. })
        || inputs.is_some_and(|inputs| inputs.hole_rims);
    let item_floors = match inputs {
        Some(inputs) if per_item => inputs.face_floors.as_slice(),
        _ => &[],
    };
    // The floors are top first.
    match (item_floors.first(), item_floors.last()) {
        (Some(top), Some(bottom)) if top.z.to_bits() != bottom.z.to_bits() => {
            return format!(
                "Floors: Z {:.2} to {:.2}, {} floors. The row shows the lowest.",
                bottom.z,
                top.z,
                item_floors.len()
            );
        }
        (Some(only), Some(_)) => return format!("Floor: Z {:.2}.", only.z),
        _ => {}
    }
    let z = match refs.map(|refs| refs.floor_z(floor)) {
        Some(Ok(z)) => format!("Floor: Z {z:.2}."),
        Some(Err(refusal)) => return format!("No floor Z: {}", floor_reason(refusal)),
        None => return "No floor Z: the heights of the toolpath do not resolve.".to_owned(),
    };
    // GT-4: the face that the Geometry tab slot holds.
    match (floor, refs.and_then(|refs| refs.floor_face_z)) {
        (Floor::Face { .. }, Some(face_z)) => {
            format!("{z} {}: face Z {face_z:.2}.", floor.reference_label())
        }
        _ => z,
    }
}

/// D6, W3d: the Bottom height row of an `OpFloor` op (Pocket, Profile,
/// Zigzag, Adaptive 2D, Bore, Drill; geometry selection P3.7, P6.6, R-B).
/// It is one `PropRow` in the column of the height rows:
///
/// - the value is the resolved floor Z (absolute, as on every height row
///   and in the diagram). An edit writes the floor in its reference
///   (`FloorReferences::at_z`, ruling L-10), as a diagram drag does. The
///   domain of the door refuses a bad value before it reaches the config.
///   Under "Each picked face" and "Each hole floor" the value is the
///   lowest floor; the range of the floors is on the hover (FR-2 F25);
/// - the combo holds the references the op admits
///   (`Floor::row_references`; UX-8: no "A picked face"). A change of
///   reference keeps the floor Z (`FloorReferences::re_express`). An entry
///   with no Z here, or whose value the domain refuses, is disabled with
///   its reason. With no resolved heights the combo is disabled;
/// - S11: an unset floor shows an empty field. The hint "set a value" does
///   not fit the value field, so the field shows "—" and the hint is on
///   its hover (`components::optional_number_field`, W3f).
///   A typed Z sets the floor from the Top height;
/// - G-DEPTHSTOCK: a floor below the stock bottom is the Caution state of
///   the row, its detail on the hover.
fn draw_bottom_height(
    ui: &mut egui::Ui,
    op: OperationType,
    bottom: BottomHeight<'_>,
    mut state: RowState,
) {
    let BottomHeight {
        floor,
        inputs,
        depth_caution,
    } = bottom;
    let refs = inputs.map(|inputs| inputs.refs);
    if let Some(caution) = depth_caution {
        // F1.18: the hover names the floor and the stock, never the Bottom
        // Z of the Heights config, which moves no motion on these ops.
        state.raise(
            Role::Caution,
            &format!(
                "{}. The cut floor sits {:.2} mm below the bottom of a {:.2} mm board, so \
                 the tool cuts into the bed. Generate stays enabled. Check the floor, or the \
                 stock thickness on the Stock panel.",
                caution.message(),
                caution.excess_mm,
                caution.stock_thickness_mm
            ),
        );
    }
    let current = floor.clone();
    let resolved = refs.map(|refs| refs.floor_z(&current));
    if let Some(Err(refusal)) = resolved
        && !matches!(refusal, FloorRefusal::Unset)
    {
        state.raise(
            Role::Danger,
            &format!("No floor Z: {}", floor_reason(refusal)),
        );
    }
    let hover = format!(
        "{FLOOR_FROM_HOVER}\n\n{}: {}\n{}",
        current.reference_label(),
        current.reference_help(),
        resolved_floor_text(&current, inputs)
    );
    let mut typed: Option<f64> = None;
    let mut picked: Option<Floor> = None;
    let out = PropRow::new(BOTTOM_HEIGHT_LABEL)
        .state(state.role)
        .hover(Some(&hover))
        .detail(state.detail.as_deref())
        .show(ui, |ui| {
            let size = height_value_size(ui);
            // S11: with no Z to show, the field is empty. Only an unset
            // floor takes a typed Z; a reference with no Z here takes a new
            // reference first.
            let unset = matches!(resolved, Some(Err(FloorRefusal::Unset)));
            let empty = if unset {
                rs_cam_core::compute::not_set::SET_A_VALUE_HINT
            } else {
                "no Z"
            };
            let spec = NumberSpec::new(0.1).decimals(2).empty(empty).unit("mm");
            let mut shown = resolved.and_then(Result::ok);
            let editable = shown.is_some() || unset;
            let value = ui
                .add_enabled_ui(editable, |ui| {
                    let (response, changed) = optional_number_field(ui, &mut shown, &spec, size);
                    if changed {
                        typed = shown;
                    }
                    response
                })
                .inner;
            let _ = ui
                .add_enabled_ui(refs.is_some(), |ui| {
                    egui::ComboBox::from_id_salt(("floor_from", op.name()))
                        .truncate()
                        .width(ui.available_width())
                        .selected_text(floor_display_label(&current))
                        .show_ui(ui, |ui| {
                            // UX-8: "A picked face" is not offered here.
                            // "Use as floor" in the Geometry tab is its one
                            // door (L-R5); a change away from it is offered.
                            for template in Floor::row_references(op.capabilities().item_use) {
                                let selected = current.same_reference(&template);
                                let next = match refs {
                                    Some(refs) if !selected => {
                                        Some(refs.re_express(&current, template.clone()))
                                    }
                                    Some(_) | None => None,
                                };
                                let enabled = selected || matches!(next, Some(Ok(_)));
                                let response = ui
                                    .add_enabled(
                                        enabled,
                                        egui::Button::selectable(
                                            selected,
                                            template.reference_label(),
                                        ),
                                    )
                                    .on_hover_text(template.reference_help())
                                    .on_disabled_hover_text(match next {
                                        Some(Err(refusal)) => floor_reason(refusal),
                                        Some(Ok(_)) | None => "",
                                    });
                                if response.clicked()
                                    && let Some(Ok(re_expressed)) = next
                                {
                                    picked = Some(re_expressed);
                                }
                            }
                        });
                })
                .response
                .on_disabled_hover_text(
                    "The heights of the toolpath do not resolve, so no floor Z is kept.",
                );
            value
        });
    let _ = out.inner.on_hover_text(&out.hover);
    if let (Some(z), Some(refs)) = (typed, refs) {
        // S11: an unset floor has no reference; a typed Z sets it from the
        // Top height, as `Floor::with_value` does for a typed depth.
        let like = if current.is_unset() {
            Floor::Top { depth: 0.0 }
        } else {
            current.clone()
        };
        if let Ok(moved) = refs.at_z(like, z) {
            *floor = moved;
        }
    }
    if let Some(next) = picked {
        *floor = next;
    }
}

// ── Stepover Pattern Diagram ─────────────────────────────────────────────

// The thirteen diagram renderers moved to `shape_diagrams.rs` (P4 split).
// The banner above marks the end of the Heights panel section.

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use rs_cam_core::mesh::make_test_flat;
    use rs_cam_core::polygon::Polygon2;
    use rs_cam_core::session::ProjectSessionBuilder;

    use super::*;
    use crate::state::job::{ModelId, ModelKind, ModelUnits, ToolConfig, ToolId, ToolType};
    use crate::state::toolpath::{OperationConfig, OperationType, ToolpathEntry, ToolpathId};

    // ── G-HEIGHTSTAB: the Heights tab is a pure read ──────────────────
    //
    // These pin the two halves of the row contract: the display projection
    // cannot write (it takes `&HeightMode`), and the commit is gated on a
    // real user edit. See `height_row_display`'s doc comment for the defect.

    /// Stock 0..25, and an operation with NO depth semantics — the whole 3D
    /// family (`DepthSemantics::None` → `default_depth_for_heights() == 0`).
    /// This is the exact shape that took the adaptive3d rough to zero moves.
    fn zero_depth_3d_ctx() -> HeightContext {
        HeightContext {
            safe_z: 30.0,
            op_depth: 0.0,
            stock_top_z: 25.0,
            stock_bottom_z: 0.0,
            model_top_z: Some(24.0),
            model_bottom_z: Some(2.0),
            floor: None,
        }
    }

    #[test]
    fn heights_display_leaves_auto_rows_unpinned_g_heightstab() {
        let heights = HeightsConfig::default();
        let ctx = zero_depth_3d_ctx();
        let auto = rs_cam_core::session::ToolpathHeights::from_context(&heights, ctx).heights;

        // Pre-condition: an all-Auto config leaves the floor to the operation.
        assert!(!auto.bottom_pinned);
        assert!(!auto.top_pinned);
        // …and on a zero-depth op the resolver's Auto bottom sits at the
        // MODEL bottom (R3, Corne case 2026-09-18). It used to sit AT the
        // stock top, which is exactly why committing it clipped the op to
        // nothing; the display still shows whatever the resolver returns.
        let model_bottom = ctx.model_bottom_z.expect("fixture carries a model");
        assert!((auto.bottom_z - model_bottom).abs() < 1e-9);
        assert!((auto.top_z - ctx.stock_top_z).abs() < 1e-9);

        // Rendering projects each Auto row for display only.
        for (mode, default_z) in [
            (&heights.clearance_z, auto.clearance_z),
            (&heights.retract_z, auto.retract_z),
            (&heights.feed_z, auto.feed_z),
            (&heights.top_z, auto.top_z),
            (&heights.bottom_z, auto.bottom_z),
        ] {
            let display = height_row_display(mode, HeightReference::StockTop, default_z, &ctx);
            assert_eq!(display.reference, HeightReference::StockTop);
            // The displayed "= z" hint must equal what the core resolver
            // produces — the panel must not carry its own defaults.
            let shown = display.reference.resolve_z(&ctx) + display.offset;
            assert!(
                (shown - default_z).abs() < 1e-9,
                "displayed {shown} != resolved {default_z}"
            );
        }

        // The stored config is untouched, so nothing goes stale and the
        // operation keeps deciding its own floor.
        assert!(heights.clearance_z.is_auto());
        assert!(heights.retract_z.is_auto());
        assert!(heights.feed_z.is_auto());
        assert!(heights.top_z.is_auto());
        assert!(heights.bottom_z.is_auto());
        assert!(
            !rs_cam_core::session::ToolpathHeights::from_context(&heights, ctx)
                .heights
                .bottom_pinned
        );
    }

    #[test]
    fn height_row_commits_only_on_a_user_edit_g_heightstab() {
        let ctx = zero_depth_3d_ctx();
        let mut heights = HeightsConfig::default();
        let auto = rs_cam_core::session::ToolpathHeights::from_context(&heights, ctx).heights;
        let display = height_row_display(
            &heights.bottom_z,
            HeightReference::StockTop,
            auto.bottom_z,
            &ctx,
        );

        // Viewing the tab: no widget moved.
        commit_height_row(&mut heights.bottom_z, display, false);
        assert!(heights.bottom_z.is_auto());
        assert!(
            !rs_cam_core::session::ToolpathHeights::from_context(&heights, ctx)
                .heights
                .bottom_pinned
        );

        // The user drags the row: now it pins, and only now.
        let edited = ReferenceOffset {
            reference: HeightReference::StockTop,
            offset: -6.0,
        };
        commit_height_row(&mut heights.bottom_z, edited, true);
        assert!(matches!(
            heights.bottom_z,
            HeightMode::FromReference(r) if (r.offset + 6.0).abs() < 1e-9
        ));
        let resolved = rs_cam_core::session::ToolpathHeights::from_context(&heights, ctx).heights;
        assert!(resolved.bottom_pinned);
        assert!((resolved.bottom_z - 19.0).abs() < 1e-9);
    }

    #[test]
    fn manual_height_rows_display_against_the_nearest_reference() {
        let ctx = zero_depth_3d_ctx();
        // 2.4 is nearest ModelBottom (2.0), not StockBottom (0.0).
        let mode = HeightMode::Manual(2.4);
        let display = height_row_display(&mode, HeightReference::StockTop, 0.0, &ctx);
        assert_eq!(display.reference, HeightReference::ModelBottom);
        assert!((display.offset - 0.4).abs() < 1e-9);
        // Still Manual — projecting it for display does not rewrite it.
        assert!(matches!(mode, HeightMode::Manual(v) if (v - 2.4).abs() < 1e-9));
    }

    /// W3d / G-HEIGHTSTAB: the `PropRow` rows draw an all-Auto config for
    /// several frames and write nothing, so the Bottom stays unpinned.
    #[test]
    fn drawing_the_height_rows_writes_nothing_w3d() {
        let hctx = zero_depth_3d_ctx();
        let answer =
            rs_cam_core::session::ToolpathHeights::from_context(&HeightsConfig::default(), hctx);
        let mut heights = HeightsConfig::default();
        let mut refusal = crate::state::panels::HeightsRefusal::default();
        let findings = FieldFindings::new();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                draw_heights_params(
                    ui,
                    &mut heights,
                    Some(&answer),
                    OperationType::Waterline,
                    None,
                    &findings,
                    &mut refusal,
                );
            });
            out.textures_delta.clear();
        }
        assert_eq!(heights, HeightsConfig::default());
        assert!(
            !rs_cam_core::session::ToolpathHeights::from_context(&heights, hctx)
                .heights
                .bottom_pinned
        );
    }

    fn session_polygon_model(id: usize) -> rs_cam_core::session::LoadedModel {
        rs_cam_core::session::LoadedModel {
            id: rs_cam_core::ids::ModelId::from_raw(id),
            path: PathBuf::from("demo.svg"),
            name: "2D".to_owned(),
            kind: Some(ModelKind::Svg),
            mesh: None,
            items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
                Polygon2::rectangle(-10.0, -10.0, 10.0, 10.0),
            ])),
            enriched_mesh: None,
            units: Some(ModelUnits::Millimeters),
            winding_report: None,
            load_error: None,
            import_options: Default::default(),
            import_report: None,
            drawing: Default::default(),
        }
    }

    fn session_mesh_model(id: usize) -> rs_cam_core::session::LoadedModel {
        rs_cam_core::session::LoadedModel {
            id: rs_cam_core::ids::ModelId::from_raw(id),
            path: PathBuf::from("demo.stl"),
            name: "3D".to_owned(),
            kind: Some(ModelKind::Stl),
            mesh: Some(Arc::new(make_test_flat(20.0))),
            items: Default::default(),
            enriched_mesh: None,
            units: Some(ModelUnits::Millimeters),
            winding_report: None,
            load_error: None,
            import_options: Default::default(),
            import_report: None,
            drawing: Default::default(),
        }
    }

    fn sample_tool(id: ToolId, tool_type: ToolType, diameter: f64) -> ToolConfig {
        let mut tool = ToolConfig::new_default(id, tool_type);
        tool.diameter = diameter;
        tool
    }

    fn make_session_toolpath_config(
        name: &str,
        tool_id: usize,
        model_id: usize,
        op: OperationConfig,
    ) -> rs_cam_core::session::ToolpathConfig {
        rs_cam_core::session::ToolpathConfig {
            shoe: Default::default(),
            id: rs_cam_core::ToolpathId::from_raw(0), // assigned by session.add_toolpath
            name: name.to_owned(),
            enabled: true,
            operation: op,
            dressups: Default::default(),
            heights: Default::default(),
            tool_id: Some(rs_cam_core::ids::ToolId::from_raw(tool_id)),
            model_id: rs_cam_core::ids::ModelId::from_raw(model_id),
            pre_gcode: None,
            post_gcode: None,
            boundary: Default::default(),
            boundary_inherit: true,
            stock_source: Default::default(),
            coolant: Default::default(),
            debug_options: Default::default(),
            feeds_provenance: Default::default(),
            rest_analysis: Default::default(),
            planner_origin: None,
        }
    }

    #[test]
    fn validate_toolpath_rejects_wrong_geometry_type() {
        // The builder keeps the ids this fixture chose. `add_tool` and
        // `add_model` overwrite them.
        let session = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId::from_raw(1), ToolType::EndMill, 6.0))
            .model(session_mesh_model(2))
            .build();

        let entry = ToolpathEntry::for_operation(
            ToolpathId::from_raw(3),
            "Pocket".to_owned(),
            ToolId::from_raw(1),
            ModelId::from_raw(2),
            OperationType::Pocket,
        );

        let errs = validate_toolpath(&entry, &ToolpathValidationContext::from_session(&session));
        assert!(
            errs.iter().any(|err| err.contains("2D geometry")),
            "expected 2D geometry validation error, got {errs:?}"
        );
    }

    /// F117: the GUI Generate gate and core's static check read one
    /// stepover rule, on every operation with a stepover. At equality
    /// neither refuses (Suggest writes that value); above it both do. The
    /// GUI refused `>=` on Pocket and Adaptive alone, and core flagged `>`
    /// on every op.
    #[test]
    fn the_gui_gate_and_core_read_one_stepover_rule_f117() {
        use rs_cam_core::diagnostics::adapters::from_static_checks::diagnostics_from_static_checks;
        use rs_cam_core::diagnostics::ids::GEOM_STEPOVER_EXCEEDS_DIAMETER;

        const DIAMETER: f64 = 6.0;
        let tool = sample_tool(ToolId::from_raw(1), ToolType::EndMill, DIAMETER);
        let session = ProjectSessionBuilder::new()
            .tool(tool.clone())
            .model(session_mesh_model(2))
            .build();
        let ctx = ToolpathValidationContext::from_session(&session);

        let mut checked = 0;
        for &op_type in OperationType::ALL {
            let mut op = OperationConfig::new_default(op_type);
            if op.stepover().is_none() {
                continue;
            }
            checked += 1;
            for (stepover, refused) in [(DIAMETER, false), (DIAMETER * 1.5, true)] {
                assert!(op.set_stepover(stepover));
                let tc = make_session_toolpath_config("Op", 1, 2, op.clone());
                let gui = validate_toolpath_config(&tc, &ctx)
                    .iter()
                    .any(|e| e == super::registry::STEPOVER_EXCEEDS_DIAMETER_REFUSAL);
                let core = diagnostics_from_static_checks(tc.id, &op, &tool, None)
                    .iter()
                    .any(|d| d.id.as_str() == GEOM_STEPOVER_EXCEEDS_DIAMETER);
                assert_eq!(
                    (gui, core),
                    (refused, refused),
                    "{op_type:?} at stepover {stepover} on a {DIAMETER} mm tool: \
                     the GUI gate and core must give one answer"
                );
            }
        }
        // Non-vacuity: Pocket and Adaptive and at least one other op.
        assert!(checked > 2, "only {checked} ops carry a stepover");
    }

    /// A drawing with one circle-centre target — what a DXF with a `CIRCLE`
    /// entity imports to.
    fn session_target_model(id: usize) -> rs_cam_core::session::LoadedModel {
        use rs_cam_core::selection::{Item, ItemSet, Shape, SourceRef};
        let mut model = session_polygon_model(id);
        let mut items: Vec<Item> = model.items.iter().cloned().collect();
        items.push(
            Item::new(
                SourceRef::ToolpathModel,
                Some("holes".into()),
                Shape::circle(rs_cam_core::geo::P2::new(0.0, 0.0), 6.0),
            )
            .unwrap(),
        );
        model.items = Arc::new(ItemSet::new(items));
        model
    }

    fn drill_entry(model: usize) -> ToolpathEntry {
        ToolpathEntry::for_operation(
            ToolpathId::from_raw(3),
            "Drill".to_owned(),
            ToolId::from_raw(1),
            ModelId::from_raw(model),
            OperationType::Drill,
        )
    }

    /// The session of the drill validation tests: tool 1, `model`, and a
    /// Drill toolpath (id 3, the id of [`drill_entry`]) with `selection`.
    fn drill_session(
        model: rs_cam_core::session::LoadedModel,
        selection: rs_cam_core::selection::Selection,
    ) -> (
        rs_cam_core::session::ProjectSession,
        rs_cam_core::session::ToolpathConfig,
    ) {
        // 6 mm: the circle of `session_target_model` (P6.5, O-3: a Drill
        // drills a circle of the tool size only).
        drill_session_with_tool(model, selection, 6.0)
    }

    /// [`drill_session`] with a tool of `tool_diameter` (mm).
    fn drill_session_with_tool(
        model: rs_cam_core::session::LoadedModel,
        selection: rs_cam_core::selection::Selection,
        tool_diameter: f64,
    ) -> (
        rs_cam_core::session::ProjectSession,
        rs_cam_core::session::ToolpathConfig,
    ) {
        let mut tc = make_session_toolpath_config(
            "Drill",
            1,
            model.id.raw(),
            OperationConfig::Drill(crate::state::toolpath::DrillConfig {
                selection,
                ..Default::default()
            }),
        );
        tc.id = ToolpathId::from_raw(3);
        let session = ProjectSessionBuilder::new()
            .tool(sample_tool(
                ToolId::from_raw(1),
                ToolType::EndMill,
                tool_diameter,
            ))
            .model(model)
            .toolpath(tc.clone())
            .build();
        (session, tc)
    }

    /// G-DRILLCENTROID (UX-R03-004), geometry selection P2.3c: a Drill op
    /// on a drawing with closed shapes but no circles or points is blocked
    /// with the generator's own sentence (`Empty`). Pre-fix the validator
    /// was silent and Generate drilled the polygon centroid.
    #[test]
    fn validate_drill_blocks_when_model_holds_no_hole() {
        let (session, tc) = drill_session(
            session_polygon_model(2),
            rs_cam_core::selection::Selection::drawing(),
        );
        let ctx = ToolpathValidationContext::from_session(&session);
        let errs = validate_toolpath(&drill_entry(2), &ctx);
        assert!(
            errs.iter()
                .any(|e| e.starts_with("Drill:") && e.contains("nothing is left to cut")),
            "expected the generator's Empty refusal, got {errs:?}"
        );

        // The session-config door prints the same sentence.
        let config_errs = validate_toolpath_config(&tc, &ctx);
        assert_eq!(
            config_errs, errs,
            "both doors print the generator's sentence"
        );
    }

    #[test]
    fn validate_drill_passes_when_model_holds_a_circle() {
        let (session, _) = drill_session(
            session_target_model(2),
            rs_cam_core::selection::Selection::drawing(),
        );
        let errs = validate_toolpath(
            &drill_entry(2),
            &ToolpathValidationContext::from_session(&session),
        );
        assert!(
            !errs.iter().any(|e| e.starts_with("Drill:")),
            "a model with a circle must not be blocked: {errs:?}"
        );
    }

    /// Geometry selection P6.5 (O-3): with a 3 mm tool the 6 mm circle is
    /// not of the tool size. The Drill leaves it out, nothing is left, and
    /// the validator blocks Generate with the generator's sentence, which
    /// names the circle and the fit.
    #[test]
    fn validate_drill_blocks_a_circle_that_is_not_the_tool_size_p6_5() {
        let (session, _) = drill_session_with_tool(
            session_target_model(2),
            rs_cam_core::selection::Selection::drawing(),
            3.0,
        );
        let errs = validate_toolpath(
            &drill_entry(2),
            &ToolpathValidationContext::from_session(&session),
        );
        assert!(
            errs.iter().any(|e| e.starts_with("Drill:")
                && e.contains("nothing is left to cut")
                && e.contains("is larger than the tool diameter 3.000 mm")),
            "a circle that is not the tool size blocks Generate: {errs:?}"
        );
    }

    /// G-DRILLPICKSTALE on the selection: a pick that names no item of the
    /// drawing blocks Generate with the `LostPick` sentence.
    #[test]
    fn validate_drill_blocks_a_lost_pick() {
        use rs_cam_core::selection::{ItemKind, MatchKey, Selection, SourceRef};
        let lost = MatchKey {
            source: SourceRef::ToolpathModel,
            kind: ItemKind::Circle,
            layer: Some("holes".to_owned()),
            anchor: [50.0, 50.0],
            measure: 6.0,
            brep: None,
            z: None,
        };
        let (session, _) =
            drill_session(session_target_model(2), Selection::from_picks(vec![lost]));
        let errs = validate_toolpath(
            &drill_entry(2),
            &ToolpathValidationContext::from_session(&session),
        );
        assert!(
            errs.iter()
                .any(|e| e.contains("no item of the drawing matches this pick")),
            "a lost pick blocks Generate: {errs:?}"
        );
    }
}
