use rs_cam_core::compute::alignment_pins::{
    NOMINAL_DOWEL_MM, PIN_MATCH_TOL_MM, PinObstacle, PinPlacementError, pin_tool_finding,
};

use rs_cam_core::geo::BoundingBox3;

use super::PanelEdit;
use crate::state::job::{AlignmentPin, FaceUp, StockConfig, ZRotation};
use crate::ui::AppEvent;
use crate::ui::components::UiExt as _;
use crate::ui::components::ValueRow;
use crate::ui::components::text;

/// Reconcile a finished stock draft with the `auto_from_model` rule.
///
/// R12 (Corne case §1 footnote): a saved project carried
/// `auto_from_model = true` beside dimensions that no bounding box
/// produced. The MCP `set_stock_config` door already clears the flag when
/// a dimension or an origin is set explicitly; the panel let the operator
/// drag a dimension under a ticked checkbox, and the next load re-derived
/// the numbers from the model. This is the one rule both surfaces follow:
///
/// - A dimension or an origin that differs from `previous` is a manual
///   stock. The flag clears and the numbers stand.
/// - Otherwise a draft under `auto_from_model` refits to `model_bbox`.
///   That covers the flag flipped on, a padding edit under auto, and a
///   record whose stored numbers lag the model.
/// - Without a model the flag keeps its value and nothing refits.
///
/// `previous` is the session's record, which the draft was cloned from
/// when the edit began.
#[must_use]
pub fn reconcile_auto_from_model(
    previous: &StockConfig,
    mut draft: StockConfig,
    model_bbox: Option<&BoundingBox3>,
) -> StockConfig {
    let geometry_edited = draft.x != previous.x
        || draft.y != previous.y
        || draft.z != previous.z
        || draft.origin_x != previous.origin_x
        || draft.origin_y != previous.origin_y
        || draft.origin_z != previous.origin_z;
    if geometry_edited {
        draft.auto_from_model = false;
        return draft;
    }
    if draft.auto_from_model
        && let Some(bbox) = model_bbox
    {
        draft.update_from_bbox(bbox);
    }
    draft
}

/// Draw the stock panel over a SCRATCH copy of the stock configuration.
///
/// WP6: `stock` is a draft the caller owns, not the session's own record.
/// The caller applies `Command::SetStockConfig` when the returned
/// [`PanelEdit`] reports a finished edit. The panel therefore pushes no
/// `StockChanged` event of its own; the command carries the invalidation
/// the event used to ask for.
///
/// `has_model` says whether a model with geometry is loaded. It gates
/// the caption under the checkbox, because "refit to the model" is not
/// an offer the panel can keep without one.
///
/// `setups` holds the in-plane map of each setup (`face_up` and
/// `z_rotation`), in setup order. The pin section judges the pins against
/// every one of them.
///
/// `pin_inputs` holds the pin obstacles (the part models, the keep-outs
/// and the fixtures) and the diameter of the tool that drills the pin
/// holes. The "Auto-place keyed pins" button plans from them, as the
/// "Two-sided setup" route does. `pin_plan_memo` holds the last plan, so
/// a frame that changes no input does not run the placer again.
pub fn draw(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    setups: &[(FaceUp, ZRotation)],
    has_model: bool,
    pin_inputs: &KeyedPinInputs,
    pin_plan_memo: &mut crate::state::panels::KeyedPinPlanMemo,
    events: &mut Vec<AppEvent>,
) -> PanelEdit {
    ui.heading("Stock Setup");
    ui.separator();

    let mut edit = PanelEdit::default();

    // Material picker — hierarchical menu so the backend enum shape
    // (flat list of variants) doesn't leak into the UX. Drills
    // Wood ▶ → Softwood/Hardwood ▶ → species; flat leaves for
    // Plywood / Sheet / Plastic / Aluminum / Foam. Wood category
    // merges curated WoodSpecies + wood_species_library() (132
    // additional species, FPL Ch.5 + Wood Database).
    ui.add_space(4.0);
    if draw_hierarchical_material_picker(ui, &mut stock.material) {
        edit.commit();
    }

    // Show material properties (read-only)
    ui.param_grid("material_info", |ui| {
            ui.label(
                egui::RichText::new("Hardness Index:")
                    .small()
                    .color(crate::ui::tokens::TEXT_MUTED),
            ).on_hover_text(
                "Per-material feed-rate scaling factor (softwood baseline = 1.0). Higher values reduce recommended feed rates and depths of cut."
            );
            ui.label(
                egui::RichText::new(format!("{:.2}", stock.material.feed_scale_factor()))
                    .small()
                    .color(crate::ui::tokens::TEXT_MUTED),
            );
            ui.end_row();

            ui.label(
                egui::RichText::new("Force line:")
                    .small()
                    .color(crate::ui::tokens::TEXT_MUTED),
            ).on_hover_text(
                "The material's cutting-force line Fc/ap = Ks \u{00B7} h + F_edge (ruling B6). The force and power models read it. A material with no measured line shows \u{2014}."
            );
            // One line per material: the Curti 2021 density law, the Goli
            // 2018 MDF line, or a named refusal (ruling B6).
            let force_line = stock.material.force_line();
            let (_, force_hover) = rs_cam_core::material::force_line::card_lines(&force_line);
            let force_text = match &force_line {
                Ok(line) => format!(
                    "Ks {:.1} N/mm\u{00B2}, F_edge {:.2} N/mm",
                    line.ks_n_per_mm2(),
                    line.f_edge_n_per_mm()
                ),
                Err(_) => "\u{2014}".to_owned(),
            };
            ui.label(
                egui::RichText::new(force_text)
                    .small()
                    .color(crate::ui::tokens::TEXT_MUTED),
            )
            .on_hover_text(force_hover);
            ui.end_row();
        });

    ui.add_space(8.0);

    // R12: a dragged or typed dimension is a manual stock. The checkbox
    // below unticks in the same frame, so the operator sees the rule as
    // it applies; `reconcile_auto_from_model` applies the same rule at
    // the commit, so a draft cannot carry both.
    let mut geometry_edited = false;
    ui.label("Dimensions:");
    ui.param_grid("stock_dims", |ui| {
        let out = ValueRow::new("X:", &mut stock.x, " mm", 0.5, 0.1..=10000.0).show(ui);
        edit.drag(&out.value_response);
        geometry_edited |= out.value_response.changed();

        let out = ValueRow::new("Y:", &mut stock.y, " mm", 0.5, 0.1..=10000.0).show(ui);
        edit.drag(&out.value_response);
        geometry_edited |= out.value_response.changed();

        let out = ValueRow::new("Z:", &mut stock.z, " mm", 0.5, 0.1..=10000.0).show(ui);
        edit.drag(&out.value_response);
        geometry_edited |= out.value_response.changed();
    });

    ui.add_space(8.0);
    ui.label("Origin:");
    ui.param_grid("stock_origin", |ui| {
        let out =
            ValueRow::new("X:", &mut stock.origin_x, " mm", 0.5, f64::MIN..=f64::MAX).show(ui);
        edit.drag(&out.value_response);
        geometry_edited |= out.value_response.changed();

        let out =
            ValueRow::new("Y:", &mut stock.origin_y, " mm", 0.5, f64::MIN..=f64::MAX).show(ui);
        edit.drag(&out.value_response);
        geometry_edited |= out.value_response.changed();

        let out =
            ValueRow::new("Z:", &mut stock.origin_z, " mm", 0.5, f64::MIN..=f64::MAX).show(ui);
        edit.drag(&out.value_response);
        geometry_edited |= out.value_response.changed();
    });
    if geometry_edited {
        stock.auto_from_model = false;
    }

    ui.add_space(8.0);
    edit.click(&ui.checkbox(&mut stock.auto_from_model, "Auto from model"));
    if !stock.auto_from_model && has_model {
        ui.label(text::caption(
            "Stock is manual. Tick to refit to the model.",
        ));
    }
    if stock.auto_from_model {
        ui.horizontal(|ui| {
            ui.label("Padding:");
            edit.drag(
                &ui.add(
                    egui::DragValue::new(&mut stock.padding)
                        .suffix(" mm")
                        .speed(0.1)
                        .range(0.0..=100.0),
                ),
            );
        });
    }

    ui.add_space(12.0);
    edit.merge(draw_alignment_pins(
        ui,
        stock,
        setups,
        pin_inputs,
        pin_plan_memo,
        events,
    ));
    edit
}

/// Draw the "Alignment Pins" collapsible section in the stock panel.
fn draw_alignment_pins(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    setups: &[(FaceUp, ZRotation)],
    pin_inputs: &KeyedPinInputs,
    pin_plan_memo: &mut crate::state::panels::KeyedPinPlanMemo,
    events: &mut Vec<AppEvent>,
) -> PanelEdit {
    // The flip the pins register: the first setup whose flip is in plane
    // (a Bottom setup), with its full map, `face_up` and `z_rotation` (P2).
    // One core selector, so this panel, the viewport axis line and the
    // overlay precondition read one answer (P3). An edge-up setup is not
    // a flip here (P9): pins in the XY plane do not register it.
    let flip_map = stock.pin_flip_map(setups.iter().copied());
    let has_flipped_setup = flip_map.is_some();
    let header = egui::RichText::new("Alignment Pins")
        .strong()
        .color(crate::ui::tokens::TEXT_STRONG);

    // Pins only matter for two-sided work: open by default only when pins
    // exist or a flip is programmed (density pass 2026-06-11).
    egui::CollapsingHeader::new(header)
        .default_open(!stock.alignment_pins.is_empty() || has_flipped_setup)
        .show(ui, |ui| {
            let mut edit = PanelEdit::default();

            // Quick two-sided setup button
            if !has_flipped_setup && stock.alignment_pins.is_empty() {
                if ui.button("Two-sided setup").clicked() {
                    events.push(AppEvent::SetupTwoSided);
                }
                ui.label(
                    egui::RichText::new(
                        "Creates flipped Setup 2, sets flip axis, places 3 keyed pins",
                    )
                    .small()
                    .color(crate::ui::tokens::TEXT_MUTED),
                );
                ui.add_space(4.0);
            }

            // The flip axis is derived from the setups and never stored
            // (P3). The label states the line, for example `y = 32.5 mm`.
            ui.horizontal(|ui| {
                ui.label("Flip axis:");
                let axis = flip_map.as_ref().and_then(|map| map.axis_label());
                ui.label(
                    egui::RichText::new(
                        axis.unwrap_or_else(|| "None (no flipped Bottom setup)".to_owned()),
                    )
                    .strong(),
                )
                .on_hover_text(
                    "From the setups. A Bottom setup at 0 deg mirrors Y about the stock centre \
                     line; turned 180 deg it mirrors X. Each pin must land on a pin after that \
                     mirror, and no turn of the part on the dowels may seat it too.",
                );
            });
            ui.add_space(4.0);

            // Shared pin diameter (physical dowels are one size)
            if !stock.alignment_pins.is_empty() {
                // SAFETY: non-empty guard above
                #[allow(clippy::indexing_slicing)]
                let mut shared_diameter = stock.alignment_pins[0].diameter;
                ui.horizontal(|ui| {
                    ui.label("Pin diameter:");
                    let response = ui.add(
                        egui::DragValue::new(&mut shared_diameter)
                            .suffix(" mm")
                            .speed(0.1)
                            .range(1.0..=25.0),
                    );
                    if response.changed() {
                        for pin in stock.alignment_pins.iter_mut() {
                            pin.diameter = shared_diameter;
                        }
                    }
                    edit.drag(&response);
                });
                ui.add_space(4.0);
            }

            // Pin list
            let mut remove_idx: Option<usize> = None;
            let mut mirror_idx: Option<usize> = None;

            for (i, pin) in stock.alignment_pins.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("Pin {}:", i + 1));
                        ui.label("X");
                        edit.drag(
                            &ui.add(
                                egui::DragValue::new(&mut pin.x)
                                    .suffix(" mm")
                                    .speed(0.5)
                                    .range(0.0..=stock.x),
                            ),
                        );
                        ui.label("Y");
                        edit.drag(
                            &ui.add(
                                egui::DragValue::new(&mut pin.y)
                                    .suffix(" mm")
                                    .speed(0.5)
                                    .range(0.0..=stock.y),
                            ),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(48.0);
                        if has_flipped_setup && ui.small_button("Mirror").clicked() {
                            mirror_idx = Some(i);
                        }
                        // UP3, §4.5: a control that DELETES something takes
                        // the Danger variant, not a default button wearing a
                        // red word. The literal it carried, (200, 100, 100),
                        // was a fifth red on no scale.
                        if ui
                            .add(crate::ui::components::Button::danger("Remove"))
                            .clicked()
                        {
                            remove_idx = Some(i);
                        }
                    });
                    ui.add_space(2.0);
                });
            }

            // Process deferred actions (borrow after mutable iteration is done)
            if let (Some(idx), Some(map)) = (mirror_idx, flip_map.as_ref())
                && let Some(src) = stock.alignment_pins.get(idx).cloned()
            {
                // The core setup map, not a copy of it (P9).
                let (mx, my) = map.map_xy(src.x, src.y);
                let mirrored = AlignmentPin::new(mx, my, src.diameter);
                // Check if a pin already exists near the mirrored position
                let existing = stock.alignment_pins.iter_mut().enumerate().find(|(j, p)| {
                    *j != idx
                        && (p.x - mirrored.x).abs() < PIN_MATCH_TOL_MM
                        && (p.y - mirrored.y).abs() < PIN_MATCH_TOL_MM
                });
                if let Some((_j, existing_pin)) = existing {
                    // Snap the existing pin to exact mirrored position
                    existing_pin.x = mirrored.x;
                    existing_pin.y = mirrored.y;
                } else {
                    stock.alignment_pins.push(mirrored);
                }
                edit.commit();
            }

            if let Some(idx) = remove_idx {
                stock.alignment_pins.remove(idx);
                edit.commit();
            }

            // Buttons row. Two pins of one diameter never key: a half
            // turn about their midpoint puts each hole on the other dowel.
            // "Auto-place" therefore places one pin on the flip axis plus
            // a mirror pair off the axis. "+ Add Pin" adds one free pin.
            ui.add_space(4.0);
            let (plan_face, plan_turn) = flip_map
                .as_ref()
                .map_or((FaceUp::Top, ZRotation::Deg0), |map| {
                    (map.face_up(), map.z_rotation())
                });
            let fingerprint = keyed_pin_plan_fingerprint(stock, plan_face, plan_turn, pin_inputs);
            let plan = pin_plan_memo.get_or_plan(fingerprint, || {
                keyed_pin_plan(stock, plan_face, plan_turn, pin_inputs)
            });
            ui.horizontal(|ui| {
                let can_place = plan.is_ok();
                if ui
                    .add_enabled(
                        can_place,
                        egui::Button::new("Auto-place keyed pins").small(),
                    )
                    .on_hover_text(
                        "Replaces the pins with three: one on the flip axis and a mirror pair off \
                         the axis, so the part seats after the flip and in no turned position. \
                         The pins go in the clear material of the stock, with a wall to the \
                         edge, the part, the keep-outs and the fixtures. The part is every model \
                         that does not cover the whole stock. The pin diameter is the diameter \
                         of the drill that makes the pin holes.",
                    )
                    .clicked()
                    && let Ok(pins) = plan.as_ref()
                {
                    // The panel applies a pin-only edit as
                    // `SetAlignmentPins` (`stock_edit_command`).
                    stock.alignment_pins.clear();
                    stock.alignment_pins.extend(pins.iter().cloned());
                    edit.commit();
                }

                let centre = (stock.x * 0.5, stock.y * 0.5);
                // Guard the pile-up: repeated clicks used to stack pins
                // at one point.
                let centre_free = !stock.alignment_pins.iter().any(|p| {
                    (p.x - centre.0).abs() < PIN_MATCH_TOL_MM
                        && (p.y - centre.1).abs() < PIN_MATCH_TOL_MM
                });
                if ui
                    .add_enabled(centre_free, egui::Button::new("+ Add Pin").small())
                    .on_hover_text(
                        "Adds one pin at the stock centre; drag it into place. Pins of one \
                         diameter key only as one pin on the flip axis plus mirror pairs off the \
                         axis (not in a line, not an equilateral triangle). Two pins of different \
                         diameters also key.",
                    )
                    .clicked()
                {
                    let diameter = panel_pin_diameter(stock, pin_inputs);
                    stock
                        .alignment_pins
                        .push(AlignmentPin::new(centre.0, centre.1, diameter));
                    edit.commit();
                }
            });

            // Standing refusal, not a transient toast: if no keyed pattern
            // exists for this blank the operator needs to see why every
            // time they look, because the alternative is a placement that
            // hangs off the stock (Ø6 pin in a 5 mm ring) or one that
            // seats turned.
            // ...but only once a flip is actually programmed. With no
            // flipped setup there is nothing to key, and "cannot place
            // pins for a Top setup" would be noise.
            if has_flipped_setup && let Err(reason) = plan.as_ref() {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(reason.to_string())
                        .small()
                        .color(crate::ui::tokens::DANGER),
                );
            }

            // Flip validation — the single core check, so the GUI and any
            // load path say the same thing about the same pins. Replaces
            // the old "not symmetric about the flip axis" label, which
            // passed wanaka's diagonal pins: they were symmetric under a
            // 180 deg ROTATION, which is not a flip, and neither hole
            // would have landed on a dowel.
            // The walls (P6) and the pin tool are judged against the same
            // inputs the placer reads.
            let findings = stock
                .pin_findings_for_setups(setups.iter().copied())
                .into_iter()
                .chain(stock.pin_wall_findings(&pin_inputs.obstacles))
                .chain(pin_tool_finding(
                    &stock.alignment_pins,
                    pin_inputs.pin_tool_diameter,
                ));
            for finding in findings {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(finding.text)
                        .small()
                        .color(crate::ui::tokens::CAUTION),
                );
            }

            edit
        })
        .body_returned
        .unwrap_or_default()
}

/// Diameter of a pin that "+ Add Pin" adds.
///
/// The diameter of the first stored pin, so the pins stay one size. With
/// no pins, the diameter of the tool that drills the pin holes. With no
/// such tool either, [`NOMINAL_DOWEL_MM`].
fn panel_pin_diameter(stock: &StockConfig, pin_inputs: &KeyedPinInputs) -> f64 {
    stock
        .alignment_pins
        .first()
        .map(|p| p.diameter)
        .or(pin_inputs.pin_tool_diameter)
        .unwrap_or(NOMINAL_DOWEL_MM)
}

/// The session values the keyed-pin planner reads.
///
/// ONE constructor for both doors: the stock panel's "Auto-place keyed
/// pins" and the controller's "Two-sided setup". Before G-PINPANEL the
/// panel took its clear strips from `padding` and a pin diameter from the
/// stored pins. A hand-sized stock (380 mm wide, origin x -15, model x
/// 0..350, padding 5) then read a 5 mm strip where the real strip is
/// 15 mm, and the panel refused a pair that the other door placed.
#[derive(Debug, Clone, Default)]
pub struct KeyedPinInputs {
    /// The world-frame outlines a pin keeps a wall to: the default part
    /// models, the keep-out zones and the fixtures
    /// ([`rs_cam_core::session::ProjectSession::pin_obstacles`] of
    /// [`rs_cam_core::session::ProjectSession::default_pin_part_models`]).
    pub obstacles: Vec<PinObstacle>,
    /// The diameter of the tool that drills the pin holes
    /// ([`rs_cam_core::session::ProjectSession::pin_drill_tool_diameter`]).
    /// `None` when no pin-drill or drill operation exists.
    pub pin_tool_diameter: Option<f64>,
}

impl KeyedPinInputs {
    #[must_use]
    pub fn from_session(session: &rs_cam_core::session::ProjectSession) -> Self {
        Self {
            obstacles: session.pin_obstacles(&session.default_pin_part_models()),
            pin_tool_diameter: session.pin_drill_tool_diameter(),
        }
    }

    /// The diameter the placer gives the pins: the tool that drills the
    /// holes, else the first stored pin, else [`NOMINAL_DOWEL_MM`]. A
    /// stored pin that differs from the tool is a finding
    /// (`pin_tool_finding`), not a silent change.
    #[must_use]
    pub fn plan_pin_diameter(&self, stock: &StockConfig) -> f64 {
        self.pin_tool_diameter
            .or_else(|| stock.alignment_pins.first().map(|p| p.diameter))
            .unwrap_or(NOMINAL_DOWEL_MM)
    }
}

/// The keyed pins for `stock` under the flipped setup (`face_up` and
/// `z_rotation`).
///
/// Both doors call this function with [`KeyedPinInputs::from_session`], so
/// they cannot disagree about a placement. The clear area comes from the
/// real stock extent and the obstacles through
/// [`StockConfig::plan_keyed_pins`], never from `padding`.
pub fn keyed_pin_plan(
    stock: &StockConfig,
    face_up: FaceUp,
    z_rotation: ZRotation,
    inputs: &KeyedPinInputs,
) -> Result<[AlignmentPin; 3], PinPlacementError> {
    stock.plan_keyed_pins(
        face_up,
        z_rotation,
        &inputs.obstacles,
        inputs.plan_pin_diameter(stock),
    )
}

/// A hash of every input of [`keyed_pin_plan`]: the stock extent and
/// origin, the first stored pin's diameter, the setup map, the pin tool and
/// every obstacle point. The stock panel's memo
/// ([`crate::state::panels::KeyedPinPlanMemo`]) keys on it.
fn keyed_pin_plan_fingerprint(
    stock: &StockConfig,
    face_up: FaceUp,
    z_rotation: ZRotation,
    inputs: &KeyedPinInputs,
) -> u64 {
    use std::hash::{Hash as _, Hasher as _};

    let mut h = std::collections::hash_map::DefaultHasher::new();
    for v in [stock.x, stock.y, stock.origin_x, stock.origin_y] {
        v.to_bits().hash(&mut h);
    }
    // The first stored pin sets the diameter when no tool drills the holes.
    stock
        .alignment_pins
        .first()
        .map(|p| p.diameter.to_bits())
        .hash(&mut h);
    face_up.hash(&mut h);
    z_rotation.hash(&mut h);
    inputs.pin_tool_diameter.map(f64::to_bits).hash(&mut h);
    for o in &inputs.obstacles {
        o.kind.hash(&mut h);
        o.name.hash(&mut h);
        o.closed.hash(&mut h);
        for (x, y) in &o.points {
            x.to_bits().hash(&mut h);
            y.to_bits().hash(&mut h);
        }
    }
    h.finish()
}

/// Hierarchical material picker: drills `Category ▶ → species`. Wood
/// nests one level deeper (`Wood ▶ → Softwood/Hardwood ▶ → species`).
/// Replaces the pre-2026-05-31 flat ComboBox + separate wood sub-picker
/// — the previous shape leaked the backend `Material` enum's flat
/// variant list into the UX as a 33+ entry dropdown that mixed all
/// classes together. UX complaint from the first bench smoke.
///
/// Backed by [`rs_cam_core::material::Material::materials_by_category`],
/// which merges the curated catalog with `wood_species_library()` and
/// dedups by Janka anchor. Per-leaf filter for the long Softwood /
/// Hardwood lists (each has ~60+ species after dedup).
///
/// Returns `true` if a selection was made this frame (the caller records
/// a finished edit).
///
/// S4: the stock panel and the setup panel's stock-change editor share it,
/// so both pick from the one library.
pub(super) fn draw_hierarchical_material_picker(
    ui: &mut egui::Ui,
    material: &mut rs_cam_core::material::Material,
) -> bool {
    use rs_cam_core::material::{Material, MaterialCategory};

    let mut changed = false;
    let current_label = material.label();
    let current_category = material.category();

    // Janka indicator on the button — visual consistency between the
    // curated `Material::SolidWood { species }` and parametric
    // `Material::SolidWoodByJanka` variants (the parametric variant
    // gets the lbf annotation uniformly with curated species rather
    // than only showing it in the menu-item line).
    let janka_suffix = match &*material {
        Material::SolidWood { species } => Some(species.janka_lbf() as i64),
        Material::SolidWoodByJanka { janka_lbf, .. } => Some(*janka_lbf as i64),
        _ => None,
    };
    let menu_text = match janka_suffix {
        Some(j) => format!("{current_label}  ({j} lbf)  ▼"),
        None => format!("{current_label}  ▼"),
    };

    let groups = Material::materials_by_category();

    ui.horizontal(|ui| {
        ui.label("Material:");
        let response = ui.menu_button(menu_text, |ui| {
            // Wood ▶ nests Softwood + Hardwood. Other categories
            // render as direct leaves with their species list.
            let mut wood_buckets: Vec<&(MaterialCategory, Vec<(String, Material)>)> =
                groups.iter().filter(|(c, _)| c.is_wood()).collect();
            wood_buckets.sort_by_key(|(c, _)| match c {
                MaterialCategory::Softwood => 0,
                _ => 1,
            });

            if !wood_buckets.is_empty() {
                ui.menu_button("Wood  ▶", |ui| {
                    for (cat, entries) in &wood_buckets {
                        if draw_wood_subcategory(ui, *cat, entries, material) {
                            changed = true;
                            ui.close();
                        }
                    }
                });
            }

            for (cat, entries) in groups {
                if cat.is_wood() || entries.is_empty() {
                    continue;
                }
                ui.menu_button(format!("{}  ▶", cat.label()), |ui| {
                    for (label, mat) in entries {
                        let selected = *material == *mat;
                        if ui.selectable_label(selected, label).clicked() {
                            *material = mat.clone();
                            changed = true;
                            ui.close();
                        }
                    }
                });
            }
        });
        response.response.on_hover_text(format!(
            "Current: {} ({})",
            current_label,
            current_category.label()
        ));
    });

    changed
}

/// Render one Softwood/Hardwood submenu with a search filter. After
/// the Phase E library merge the Hardwood leaf alone has ~65 species
/// (60+ FPL + 7 curated), too long to scan without filtering.
/// Flat-leaf categories (plastic, aluminum, foam) stay short and
/// don't need a filter.
fn draw_wood_subcategory(
    ui: &mut egui::Ui,
    cat: rs_cam_core::material::MaterialCategory,
    entries: &[(String, rs_cam_core::material::Material)],
    material: &mut rs_cam_core::material::Material,
) -> bool {
    let mut changed = false;
    let label = cat.label();
    let filter_id = egui::Id::new(("wood_subcategory_filter", label));
    ui.menu_button(format!("{}  ▶", label), |ui| {
        let mut filter: String = ui.data(|d| d.get_temp(filter_id).unwrap_or_default());
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(
                egui::TextEdit::singleline(&mut filter)
                    .hint_text("Filter")
                    .desired_width(180.0),
            );
        });
        ui.add_space(2.0);

        let filter_lc = filter.to_lowercase();
        let visible: Vec<&(String, rs_cam_core::material::Material)> = entries
            .iter()
            .filter(|(l, _)| filter_lc.is_empty() || l.to_lowercase().contains(&filter_lc))
            .collect();

        ui.label(
            egui::RichText::new(format!("{} of {}", visible.len(), entries.len()))
                .small()
                .color(crate::ui::tokens::TEXT_MUTED),
        );
        ui.separator();

        egui::ScrollArea::vertical()
            .max_height(280.0)
            .show(ui, |ui| {
                for (entry_label, mat) in &visible {
                    let selected = *material == *mat;
                    if ui
                        .selectable_label(selected, entry_label.as_str())
                        .clicked()
                    {
                        *material = mat.clone();
                        changed = true;
                    }
                }
                if visible.is_empty() {
                    ui.label(
                        egui::RichText::new("no matches")
                            .small()
                            .italics()
                            .color(crate::ui::tokens::TEXT_MUTED),
                    );
                }
            });
        ui.data_mut(|d| d.insert_temp(filter_id, filter));
    });
    changed
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use rs_cam_core::compute::alignment_pins::PinObstacleKind;

    fn stock(x: f64, y: f64) -> StockConfig {
        StockConfig {
            x,
            y,
            ..StockConfig::default()
        }
    }

    /// The pin-drill inputs for a model at `x_min..x_max` and a tool of
    /// `pin_diameter`.
    fn inputs(x_min: f64, x_max: f64, y: f64, pin_diameter: f64) -> KeyedPinInputs {
        KeyedPinInputs {
            obstacles: vec![PinObstacle::rectangle(
                PinObstacleKind::Part,
                "model",
                (x_min, 0.0),
                (x_max, y),
            )],
            pin_tool_diameter: Some(pin_diameter),
        }
    }

    /// The live wanaka geometry. A 5 mm clear ring cannot hold a 6 mm
    /// dowel at ANY offset — 6 mm of pin plus 2 mm of wall each side
    /// needs 10 mm — so the only correct answer is a refusal. The
    /// pre-2026-08-22 controller placed the pin at x = 2.5, spanning
    /// -0.5..5.5 relative to the stock edge: hanging off the blank.
    #[test]
    fn panel_refuses_o6_pin_in_a_5mm_clear_ring() {
        let s = stock(140.0, 150.0);
        match keyed_pin_plan(
            &s,
            FaceUp::Bottom,
            ZRotation::Deg0,
            &inputs(5.0, 135.0, 150.0, 6.0),
        ) {
            Err(PinPlacementError::NoRoomOnAxis { need_mm, .. }) => {
                assert!((need_mm - 10.0).abs() < 1e-9);
            }
            other => panic!("expected a strip-too-narrow refusal, got {other:?}"),
        }
    }

    /// With room to work in, the panel places one pin on the mirror line
    /// and a mirror pair off it. The pattern seats after the flip and, by
    /// the intrinsic check, seats in no turned position (P1).
    #[test]
    fn panel_plan_seats_the_flip_and_keys_it() {
        let s = stock(140.0, 150.0);
        let pins = keyed_pin_plan(
            &s,
            FaceUp::Bottom,
            ZRotation::Deg0,
            &inputs(20.0, 120.0, 150.0, 6.0),
        )
        .unwrap();
        assert!(
            (pins[0].y - s.y * 0.5).abs() < 1e-9,
            "the axis pin is off the mirror line at y={}",
            pins[0].y
        );
        assert!(
            (pins[1].y + pins[2].y - s.y).abs() < 1e-9 && (pins[1].x - pins[2].x).abs() < 1e-9,
            "pins 2 and 3 are not a mirror pair: {pins:?}"
        );
        let mut placed = s;
        placed.alignment_pins = pins.to_vec();
        let report = placed.validate_pins_for_flip(FaceUp::Bottom, ZRotation::Deg0);
        assert!(report.seats(), "{report:?}");
        assert!(report.keyed(), "{report:?}");
        assert!(report.out_of_bounds.is_empty(), "{report:?}");
    }

    /// No flipped setup means no flip to key, and the panel says so
    /// rather than inventing a pattern.
    #[test]
    fn panel_refuses_when_no_flip_is_programmed() {
        let s = stock(140.0, 150.0);
        assert!(matches!(
            keyed_pin_plan(
                &s,
                FaceUp::Top,
                ZRotation::Deg0,
                &inputs(20.0, 120.0, 150.0, 6.0)
            ),
            Err(PinPlacementError::UnsupportedFlip { .. })
        ));
    }

    /// No tool drills the holes: the placer keeps the stored pin size, and
    /// with no stored pin it uses the stated nominal dowel. It does not
    /// refuse, and it never takes a tool that does not drill the holes.
    #[test]
    fn panel_without_a_pin_tool_keeps_the_stored_pin_size() {
        let no_tool = KeyedPinInputs {
            pin_tool_diameter: None,
            ..inputs(20.0, 120.0, 150.0, 6.0)
        };
        let mut s = stock(140.0, 150.0);
        assert!((no_tool.plan_pin_diameter(&s) - NOMINAL_DOWEL_MM).abs() < 1e-12);
        s.alignment_pins = vec![AlignmentPin::new(10.0, 75.0, 8.0)];
        assert!((no_tool.plan_pin_diameter(&s) - 8.0).abs() < 1e-12);
        let pins = keyed_pin_plan(&s, FaceUp::Bottom, ZRotation::Deg0, &no_tool).unwrap();
        assert!(
            pins.iter().all(|p| (p.diameter - 8.0).abs() < 1e-12),
            "{pins:?}"
        );
    }
}
