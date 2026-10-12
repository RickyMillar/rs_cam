use rs_cam_core::geo::BoundingBox3;

use super::PanelEdit;
use super::pins::PinSummary;
use crate::state::job::StockConfig;
use crate::ui::components::UiExt as _;
use crate::ui::components::ValueRow;

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
/// `pins` holds the words of the "Alignment pins" block (`super::pins`).
/// The block only opens the pin wizard; the wizard edits the pins. The
/// answer carries the panel's edit and the block's request.
pub fn draw(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    has_model: bool,
    pins: &PinSummary,
) -> StockPanelOut {
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
            // IC1: the two coefficients without their units, so the cell
            // fits the panel. The units are on the hover.
            let force_text = match &force_line {
                Ok(line) => format!(
                    "Ks {:.1} \u{00B7} F_edge {:.2}",
                    line.ks_n_per_mm2(),
                    line.f_edge_n_per_mm()
                ),
                Err(_) => "\u{2014}".to_owned(),
            };
            let force_hover = match &force_line {
                Ok(line) => format!(
                    "Ks {:.1} N/mm\u{00B2}, F_edge {:.2} N/mm\n{force_hover}",
                    line.ks_n_per_mm2(),
                    line.f_edge_n_per_mm()
                ),
                Err(_) => force_hover,
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
    // IC1: the manual-stock hint is the checkbox's hover, not a caption.
    let auto_hover = if has_model {
        "Fit the stock to the model. An edit to a size or an origin above \
         clears this, and the stock stays manual until you tick it again."
    } else {
        "Fit the stock to the model."
    };
    edit.click(
        &ui.checkbox(&mut stock.auto_from_model, "Auto from model")
            .on_hover_text(auto_hover),
    );
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
    let open_pin_wizard = super::pins::draw(ui, pins);
    StockPanelOut {
        edit,
        open_pin_wizard,
    }
}

/// What the stock panel asks of the caller.
#[derive(Debug, Default)]
pub struct StockPanelOut {
    /// The edit of the stock draft.
    pub edit: PanelEdit,
    /// "Set up pins…" or "Edit pins…": open the pin wizard.
    pub open_pin_wizard: bool,
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
