use rs_cam_core::compute::alignment_pins::{
    PIN_MATCH_TOL_MM, PIN_WALL_MM, PinPlacementError, PinPlacementRequest, place_keyed_pins,
};

use rs_cam_core::geo::BoundingBox3;

use super::PanelEdit;
use crate::state::job::{AlignmentPin, FaceUp, FlipAxis, StockConfig};
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
pub fn draw(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    has_flipped_setup: bool,
    has_model: bool,
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
    if draw_hierarchical_material_picker(ui, stock) {
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
    edit.merge(draw_alignment_pins(ui, stock, has_flipped_setup, events));
    edit
}

/// Draw the "Alignment Pins" collapsible section in the stock panel.
fn draw_alignment_pins(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    has_flipped_setup: bool,
    events: &mut Vec<AppEvent>,
) -> PanelEdit {
    let header = egui::RichText::new("Alignment Pins")
        .strong()
        .color(crate::ui::tokens::TEXT_STRONG);

    // Pins only matter for two-sided work: open by default only when pins
    // exist or a flip is programmed (density pass 2026-06-11). Keyed off
    // the setups rather than the cached `flip_axis`, so a project whose
    // stored cache is stale still opens the section.
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
                    egui::RichText::new("Creates flipped Setup 2, sets flip axis, places 2 pins")
                        .small()
                        .color(crate::ui::tokens::TEXT_MUTED),
                );
                ui.add_space(4.0);
            }

            // Flip axis — DERIVED FOR DISPLAY, and never written here
            // (G-PINAUTO, 2026-08-22).
            //
            // This used to be a free three-way dropdown that nothing in
            // core read, whose own doc contradicted itself, and which on
            // the live project sat at `null` beside a `FaceUp::Bottom`
            // setup. A control that can disagree with the setups is worse
            // than no control — so it now reports what the setups say,
            // and every check below keys off the flip, not off the field.
            //
            // It does NOT write the corrected value back. Opening a panel
            // is not an edit: a render-time write marks the project
            // dirty, pushes an undo entry for something the user did not
            // do, raises a save prompt after a pure inspection, and
            // silently proposes to modify a project opened for reference.
            // The stored cache is written by the explicit two-sided
            // action; correcting a stale one on load is a migration and
            // belongs in the load path.
            //
            // `Vertical` is unreachable on purpose: no `FaceUp` performs
            // an X mirror. The variant survives only so older projects
            // still load — and when one does, the mismatch is named
            // rather than papered over.
            let derived_axis = has_flipped_setup.then_some(FlipAxis::Horizontal);
            ui.horizontal(|ui| {
                ui.label("Flip axis:");
                ui.label(
                    egui::RichText::new(match derived_axis {
                        Some(fa) => fa.label(),
                        None => "None (no flipped setup)",
                    })
                    .strong(),
                )
                .on_hover_text(
                    "Derived from the setups, not set here. The CAM models one flip — Bottom, \
                     which mirrors Y about the stock centre line — so pins must be invariant \
                     under that and under nothing else.",
                );
                if stock.flip_axis != derived_axis {
                    ui.label(
                        egui::RichText::new("(derived)")
                            .small()
                            .color(crate::ui::tokens::CAUTION),
                    );
                }
            });
            // Name the stored value rather than displaying the derived
            // one as though it were what the file says.
            if stock.flip_axis != derived_axis {
                ui.label(
                    egui::RichText::new(format!(
                        "This project stores {}. The stored value is a cache with no say in \
                         whether the pins are correct; it is rewritten next time pins are \
                         placed.",
                        match stock.flip_axis {
                            Some(FlipAxis::Vertical) =>
                                "'Vertical', which no flip in this CAM performs",
                            Some(FlipAxis::Horizontal) => "'Horizontal'",
                            None => "no flip axis",
                        }
                    ))
                    .small()
                    .color(crate::ui::tokens::CAUTION),
                );
            }

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
                        if derived_axis.is_some() && ui.small_button("Mirror").clicked() {
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
            if let Some((idx, axis)) = mirror_idx.zip(derived_axis) {
                // SAFETY: idx from enumerate over alignment_pins
                #[allow(clippy::indexing_slicing)]
                let src = &stock.alignment_pins[idx];
                let mirrored = mirror_pin(src, axis, stock.x, stock.y);
                // Check if a pin already exists near the mirrored position
                let existing = stock.alignment_pins.iter_mut().enumerate().find(|(j, p)| {
                    *j != idx && (p.x - mirrored.x).abs() < 0.5 && (p.y - mirrored.y).abs() < 0.5
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
                // Removing a pin IS an edit, so writing the cache here is
                // legitimate. Keeping the invariant "flip_axis is Some
                // only while pins exist" is what makes the setup panel
                // offer "Add alignment pins for this flip" again once the
                // last one is gone.
                if stock.alignment_pins.is_empty() {
                    stock.flip_axis = None;
                }
                edit.commit();
            }

            // Buttons row. The keyed pair is the whole product here: two
            // pins on the mirror line whose x-multiset is not
            // centre-symmetric. Extra pins buy redundancy against
            // rocking, never keying — a pin at x = W/2 is its own image
            // under x -> W - x — so "+ Add Pin" is deliberately dumb and
            // "Auto-place" no longer takes a count.
            ui.add_space(4.0);
            let plan = panel_pin_plan(stock, derived_axis);
            ui.horizontal(|ui| {
                let can_place = plan.is_ok();
                if ui
                    .add_enabled(
                        can_place,
                        egui::Button::new("Auto-place keyed pair").small(),
                    )
                    .on_hover_text(
                        "Two pins on the flip's mirror line, offset so the part cannot seat 180 \
                         deg out. Clear strips are taken from the stock padding — the \
                         'Two-sided setup' button uses the real model bbox and the pin-drill \
                         tool's diameter instead.",
                    )
                    .clicked()
                    && let Ok(pins) = plan.as_ref()
                {
                    stock.alignment_pins.clear();
                    stock.alignment_pins.extend(pins.iter().cloned());
                    // Explicit edit, so the cache is written here — and
                    // now there are pins for it to describe.
                    stock.flip_axis = derived_axis;
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
                        "Adds one pin at the centre of the mirror line, then drag it. A centre \
                         pin is self-symmetric, so it adds redundancy but no keying.",
                    )
                    .clicked()
                {
                    let diameter = panel_pin_diameter(stock);
                    stock
                        .alignment_pins
                        .push(AlignmentPin::new(centre.0, centre.1, diameter));
                    stock.flip_axis = derived_axis;
                    edit.commit();
                }
            });

            // Standing refusal, not a transient toast: if no keyed pair
            // exists for this blank the operator needs to see why every
            // time they look, because the alternative is a placement that
            // hangs off the stock (Ø6 pin in a 5 mm ring) or one that
            // seats four ways.
            // ...but only once a flip is actually programmed. With no
            // flipped setup there is nothing to key, and "cannot place
            // pins for a Top setup" would be noise.
            if derived_axis.is_some()
                && let Err(reason) = plan.as_ref()
            {
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
            let flip_face = if derived_axis == Some(FlipAxis::Horizontal) {
                FaceUp::Bottom
            } else {
                FaceUp::Top
            };
            for warning in stock.validate_pins_for_flip(flip_face).warnings() {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(warning)
                        .small()
                        .color(crate::ui::tokens::CAUTION),
                );
            }

            edit
        })
        .body_returned
        .unwrap_or_default()
}

/// Pin diameter the panel plans against.
///
/// The panel cannot see the tool list, so it can only honour a diameter
/// the operator already chose. When there are no pins yet it falls back
/// to a nominal dowel — the "Two-sided setup" button, which runs in the
/// controller, sizes from the pin-drill tool instead and is the door to
/// use when the drill matters.
fn panel_pin_diameter(stock: &StockConfig) -> f64 {
    const NOMINAL_DOWEL_MM: f64 = 6.0;
    stock
        .alignment_pins
        .first()
        .map(|p| p.diameter)
        .unwrap_or(NOMINAL_DOWEL_MM)
}

/// The keyed pair for this blank, as far as the panel can see it.
///
/// **Known limitation.** `ui::properties::stock::draw` is handed only the
/// `StockConfig`, so the clear strips are taken from `padding` rather
/// than from the model bbox. When the stock is auto-sized that is exact;
/// when it was sized by hand (140x150 around a 100x100 model) padding
/// under-reports the free strip and this refuses a placement that is in
/// fact available. A conservative refusal is the safe side of that error,
/// and the controller's two-sided path uses the real bbox. Widening
/// `draw`'s signature to carry the bbox is the follow-up.
fn panel_pin_plan(
    stock: &StockConfig,
    derived_axis: Option<FlipAxis>,
) -> Result<[AlignmentPin; 2], PinPlacementError> {
    let face_up = match derived_axis {
        Some(FlipAxis::Horizontal) => FaceUp::Bottom,
        // No flipped setup, or a legacy Vertical axis no `FaceUp`
        // performs: there is no in-plane flip to key.
        _ => FaceUp::Top,
    };
    place_keyed_pins(PinPlacementRequest {
        stock_w: stock.x,
        stock_d: stock.y,
        model_x_range: Some((stock.padding, stock.x - stock.padding)),
        face_up,
        pin_diameter: panel_pin_diameter(stock),
        wall_mm: PIN_WALL_MM,
    })
}

/// Create the mirror of a pin about the flip axis.
///
/// `Horizontal` is the map `FaceUp::Bottom` performs: `y -> stock_y - y`,
/// X untouched. A pin already on `y = stock_y / 2` is its own mirror,
/// which is why the auto-placed pair needs no partner.
fn mirror_pin(pin: &AlignmentPin, axis: FlipAxis, stock_x: f64, stock_y: f64) -> AlignmentPin {
    match axis {
        // Mirror about the X centre line → Y is reflected, X preserved.
        FlipAxis::Horizontal => AlignmentPin::new(pin.x, stock_y - pin.y, pin.diameter),
        // Vertical flip: mirror about the Y centerline → X is reflected
        FlipAxis::Vertical => AlignmentPin::new(stock_x - pin.x, pin.y, pin.diameter),
    }
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
fn draw_hierarchical_material_picker(
    ui: &mut egui::Ui,
    stock: &mut crate::state::job::StockConfig,
) -> bool {
    use rs_cam_core::material::{Material, MaterialCategory};

    let mut changed = false;
    let current_label = stock.material.label();
    let current_category = stock.material.category();

    // Janka indicator on the button — visual consistency between the
    // curated `Material::SolidWood { species }` and parametric
    // `Material::SolidWoodByJanka` variants (the parametric variant
    // gets the lbf annotation uniformly with curated species rather
    // than only showing it in the menu-item line).
    let janka_suffix = match &stock.material {
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
                        if draw_wood_subcategory(ui, *cat, entries, stock) {
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
                        let selected = stock.material == *mat;
                        if ui.selectable_label(selected, label).clicked() {
                            stock.material = mat.clone();
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
    stock: &mut crate::state::job::StockConfig,
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
                    let selected = stock.material == *mat;
                    if ui
                        .selectable_label(selected, entry_label.as_str())
                        .clicked()
                    {
                        stock.material = mat.clone();
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

    fn stock(x: f64, y: f64, padding: f64, pin_diameter: f64) -> StockConfig {
        let mut s = StockConfig {
            x,
            y,
            padding,
            ..StockConfig::default()
        };
        // Seed a pin so the panel picks up the diameter it plans against.
        s.alignment_pins
            .push(AlignmentPin::new(0.0, 0.0, pin_diameter));
        s
    }

    /// The live wanaka geometry. A 5 mm padding ring cannot hold a 6 mm
    /// dowel at ANY offset — 6 mm of pin plus 2 mm of wall each side
    /// needs 10 mm — so the only correct answer is a refusal. The
    /// pre-2026-08-22 controller placed the pin at x = 2.5, spanning
    /// -0.5..5.5 relative to the stock edge: hanging off the blank.
    #[test]
    fn panel_refuses_o6_pin_in_a_5mm_padding_ring() {
        let s = stock(140.0, 150.0, 5.0, 6.0);
        match panel_pin_plan(&s, Some(FlipAxis::Horizontal)) {
            Err(PinPlacementError::StripTooNarrow {
                strip_mm,
                required_mm,
                ..
            }) => {
                assert!((strip_mm - 5.0).abs() < 1e-9);
                assert!((required_mm - 10.0).abs() < 1e-9);
            }
            other => panic!("expected a strip-too-narrow refusal, got {other:?}"),
        }
    }

    /// With room to work in, the pair sits on the mirror line and is
    /// keyed: it survives the flip and blocks the other three seatings.
    #[test]
    fn panel_plan_seats_the_flip_and_keys_it() {
        let s = stock(140.0, 150.0, 20.0, 6.0);
        let pins = panel_pin_plan(&s, Some(FlipAxis::Horizontal)).unwrap();
        for p in &pins {
            assert!(
                (p.y - s.y * 0.5).abs() < 1e-9,
                "pin off the mirror line at y={}",
                p.y
            );
        }
        let mut placed = s;
        placed.alignment_pins = pins.to_vec();
        let report = placed.validate_pins_for_flip(FaceUp::Bottom);
        assert!(report.seats(), "{report:?}");
        assert!(report.keyed(), "{report:?}");
        assert!(report.out_of_bounds.is_empty(), "{report:?}");
    }

    /// No flipped setup means no flip to key, and the panel says so
    /// rather than inventing a pattern.
    #[test]
    fn panel_refuses_when_no_flip_is_programmed() {
        let s = stock(140.0, 150.0, 20.0, 6.0);
        assert!(matches!(
            panel_pin_plan(&s, None),
            Err(PinPlacementError::UnsupportedFlip { .. })
        ));
    }
}
