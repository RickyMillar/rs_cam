use crate::state::job::{AlignmentPin, FlipAxis, StockConfig};
use crate::ui::AppEvent;

pub fn draw(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    has_flipped_setup: bool,
    events: &mut Vec<AppEvent>,
) {
    ui.heading("Stock Setup");
    ui.separator();

    let mut changed = false;

    // Material picker — hierarchical menu so the backend enum shape
    // (flat list of variants) doesn't leak into the UX. Drills
    // Wood ▶ → Softwood/Hardwood ▶ → species; flat leaves for
    // Plywood / Sheet / Plastic / Aluminum / Foam. Wood category
    // merges curated WoodSpecies + WOOD_SPECIES_LIBRARY (132
    // additional species, FPL Ch.5 + Wood Database).
    ui.add_space(4.0);
    if draw_hierarchical_material_picker(ui, stock) {
        changed = true;
        events.push(AppEvent::StockMaterialChanged);
    }

    // Show material properties (read-only)
    egui::Grid::new("material_info")
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Hardness Index:")
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 150)),
            ).on_hover_text(
                "Relative material hardness (0-1). Higher values reduce recommended feed rates and depths of cut."
            );
            ui.label(
                egui::RichText::new(format!("{:.2}", stock.material.hardness_index()))
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 150)),
            );
            ui.end_row();

            ui.label(
                egui::RichText::new("Kc:")
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 150)),
            ).on_hover_text(
                "Specific cutting force (N/mm\u{00B2}). Used to calculate spindle load and recommended feed rates. Higher Kc = harder to cut."
            );
            let kc_text = match stock.material.kc_n_per_mm2() {
                Some(kc) => format!("{kc:.1} N/mm\u{00B2}"),
                None => "—".to_owned(),
            };
            ui.label(
                egui::RichText::new(kc_text)
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 150)),
            );
            ui.end_row();
        });

    ui.add_space(8.0);

    ui.label("Dimensions:");
    egui::Grid::new("stock_dims")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("X:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.x)
                        .suffix(" mm")
                        .speed(0.5)
                        .range(0.1..=10000.0),
                )
                .changed();
            ui.end_row();

            ui.label("Y:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.y)
                        .suffix(" mm")
                        .speed(0.5)
                        .range(0.1..=10000.0),
                )
                .changed();
            ui.end_row();

            ui.label("Z:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.z)
                        .suffix(" mm")
                        .speed(0.5)
                        .range(0.1..=10000.0),
                )
                .changed();
            ui.end_row();
        });

    ui.add_space(8.0);
    ui.label("Origin:");
    egui::Grid::new("stock_origin")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("X:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.origin_x)
                        .suffix(" mm")
                        .speed(0.5),
                )
                .changed();
            ui.end_row();

            ui.label("Y:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.origin_y)
                        .suffix(" mm")
                        .speed(0.5),
                )
                .changed();
            ui.end_row();

            ui.label("Z:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.origin_z)
                        .suffix(" mm")
                        .speed(0.5),
                )
                .changed();
            ui.end_row();
        });

    ui.add_space(8.0);
    changed |= ui
        .checkbox(&mut stock.auto_from_model, "Auto from model")
        .changed();
    if stock.auto_from_model {
        ui.horizontal(|ui| {
            ui.label("Padding:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.padding)
                        .suffix(" mm")
                        .speed(0.1)
                        .range(0.0..=100.0),
                )
                .changed();
        });
    }

    if changed {
        events.push(AppEvent::StockChanged);
    }

    ui.add_space(12.0);
    draw_alignment_pins(ui, stock, has_flipped_setup, events);
}

/// Draw the "Alignment Pins" collapsible section in the stock panel.
fn draw_alignment_pins(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    has_flipped_setup: bool,
    events: &mut Vec<AppEvent>,
) {
    let header = egui::RichText::new("Alignment Pins")
        .strong()
        .color(egui::Color32::from_rgb(180, 180, 195));

    egui::CollapsingHeader::new(header)
        .default_open(true)
        .show(ui, |ui| {
            let mut changed = false;

            // Quick two-sided setup button
            if !has_flipped_setup && stock.alignment_pins.is_empty() {
                if ui.button("Two-sided setup").clicked() {
                    events.push(AppEvent::SetupTwoSided);
                }
                ui.label(
                    egui::RichText::new("Creates flipped Setup 2, sets flip axis, places 2 pins")
                        .small()
                        .color(egui::Color32::from_rgb(120, 120, 130)),
                );
                ui.add_space(4.0);
            }

            // Flip axis dropdown
            let flip_label = match stock.flip_axis {
                Some(fa) => fa.label(),
                None => "None",
            };
            ui.horizontal(|ui| {
                ui.label("Flip axis:");
                egui::ComboBox::from_id_salt("flip_axis")
                    .selected_text(flip_label)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(stock.flip_axis.is_none(), "None")
                            .clicked()
                        {
                            stock.flip_axis = None;
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                stock.flip_axis == Some(FlipAxis::Horizontal),
                                "Horizontal",
                            )
                            .clicked()
                        {
                            stock.flip_axis = Some(FlipAxis::Horizontal);
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                stock.flip_axis == Some(FlipAxis::Vertical),
                                "Vertical",
                            )
                            .clicked()
                        {
                            stock.flip_axis = Some(FlipAxis::Vertical);
                            changed = true;
                        }
                    });
            });

            ui.add_space(4.0);

            // Shared pin diameter (physical dowels are one size)
            if !stock.alignment_pins.is_empty() {
                // SAFETY: non-empty guard above
                #[allow(clippy::indexing_slicing)]
                let mut shared_diameter = stock.alignment_pins[0].diameter;
                ui.horizontal(|ui| {
                    ui.label("Pin diameter:");
                    if ui
                        .add(
                            egui::DragValue::new(&mut shared_diameter)
                                .suffix(" mm")
                                .speed(0.1)
                                .range(1.0..=25.0),
                        )
                        .changed()
                    {
                        for pin in stock.alignment_pins.iter_mut() {
                            pin.diameter = shared_diameter;
                        }
                        changed = true;
                    }
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
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut pin.x)
                                    .suffix(" mm")
                                    .speed(0.5)
                                    .range(0.0..=stock.x),
                            )
                            .changed();
                        ui.label("Y");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut pin.y)
                                    .suffix(" mm")
                                    .speed(0.5)
                                    .range(0.0..=stock.y),
                            )
                            .changed();
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(48.0);
                        if stock.flip_axis.is_some() && ui.small_button("Mirror").clicked() {
                            mirror_idx = Some(i);
                        }
                        if ui
                            .small_button(
                                egui::RichText::new("Remove")
                                    .color(egui::Color32::from_rgb(200, 100, 100)),
                            )
                            .clicked()
                        {
                            remove_idx = Some(i);
                        }
                    });
                    ui.add_space(2.0);
                });
            }

            // Process deferred actions (borrow after mutable iteration is done)
            if let Some((idx, axis)) = mirror_idx.zip(stock.flip_axis) {
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
                changed = true;
            }

            if let Some(idx) = remove_idx {
                stock.alignment_pins.remove(idx);
                changed = true;
            }

            // Buttons row
            ui.add_space(4.0);
            if ui.small_button("+ Add Pin").clicked() {
                let default_diameter = stock
                    .alignment_pins
                    .first()
                    .map(|p| p.diameter)
                    .unwrap_or(6.0);
                stock.alignment_pins.push(AlignmentPin::new(
                    stock.x / 2.0,
                    stock.y / 2.0,
                    default_diameter,
                ));
                changed = true;
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let pin_count_id = ui.id().with("auto_place_count");
                let mut count: usize =
                    ui.data_mut(|d| *d.get_persisted_mut_or(pin_count_id, 2_usize));
                if ui
                    .add(
                        egui::DragValue::new(&mut count)
                            .prefix("Pins: ")
                            .range(2..=8),
                    )
                    .changed()
                {
                    ui.data_mut(|d| d.insert_persisted(pin_count_id, count));
                }
                if ui.small_button("Auto-place").clicked() {
                    auto_place_pins(stock, count);
                    changed = true;
                }
            });

            // Symmetry warning
            if let Some(axis) = stock.flip_axis
                && !stock.alignment_pins.is_empty()
                && !pins_are_symmetric(&stock.alignment_pins, axis, stock.x, stock.y)
            {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Pins are not symmetric about the flip axis")
                        .small()
                        .color(egui::Color32::from_rgb(220, 180, 60)),
                );
            }

            // Out-of-bounds warning
            if stock
                .alignment_pins
                .iter()
                .any(|p| p.x < 0.0 || p.x > stock.x || p.y < 0.0 || p.y > stock.y)
            {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("One or more pins are outside the stock bounds")
                        .small()
                        .color(egui::Color32::from_rgb(220, 100, 100)),
                );
            }

            if changed {
                events.push(AppEvent::StockChanged);
            }
        });
}

/// Create the mirror of a pin about the flip axis.
fn mirror_pin(pin: &AlignmentPin, axis: FlipAxis, stock_x: f64, stock_y: f64) -> AlignmentPin {
    match axis {
        // Horizontal flip: mirror about the X centerline → Y is reflected
        FlipAxis::Horizontal => AlignmentPin::new(pin.x, stock_y - pin.y, pin.diameter),
        // Vertical flip: mirror about the Y centerline → X is reflected
        FlipAxis::Vertical => AlignmentPin::new(stock_x - pin.x, pin.y, pin.diameter),
    }
}

/// Place `count` pins evenly distributed in the stock margin (padding area).
// SAFETY: corners indexed by seg % 4 and (seg+1) % 4, always 0..3 into a 4-element array
#[allow(clippy::indexing_slicing)]
fn auto_place_pins(stock: &mut StockConfig, count: usize) {
    // Place pins in the center of the padding margin so they hit excess
    // stock, not the model. Fall back to 10mm if padding is too small.
    let raw_margin = if stock.padding > 2.0 {
        stock.padding / 2.0
    } else {
        10.0_f64.min(stock.x / 4.0).min(stock.y / 4.0)
    };
    let diameter = stock
        .alignment_pins
        .first()
        .map(|p| p.diameter)
        .unwrap_or(6.0);

    // Pin position is its CENTER; the pin's physical edge sits one
    // radius outside that. Ensure the centre is at least
    // `radius + EDGE_CLEARANCE` inside the stock so the pin body
    // doesn't clip the edge.
    const EDGE_CLEARANCE: f64 = 2.0;
    let radius = diameter * 0.5;
    let min_margin = radius + EDGE_CLEARANCE;
    // Don't push the margin past half the smaller stock dimension
    // (would invert the placement on tiny stock).
    let max_margin = (stock.x.min(stock.y) * 0.5).max(min_margin);
    let margin = raw_margin.max(min_margin).min(max_margin);

    stock.alignment_pins.clear();

    match stock.flip_axis {
        Some(FlipAxis::Horizontal) => {
            // Pins along the flip axis centerline (Y = stock.y/2),
            // evenly spaced from left margin to right margin.
            let cy = stock.y / 2.0;
            let x_start = margin;
            let x_end = stock.x - margin;
            if count == 1 {
                stock
                    .alignment_pins
                    .push(AlignmentPin::new(stock.x / 2.0, cy, diameter));
            } else {
                let step = (x_end - x_start) / (count - 1) as f64;
                for i in 0..count {
                    stock.alignment_pins.push(AlignmentPin::new(
                        x_start + step * i as f64,
                        cy,
                        diameter,
                    ));
                }
            }
        }
        Some(FlipAxis::Vertical) => {
            // Pins along the flip axis centerline (X = stock.x/2),
            // evenly spaced from front margin to back margin.
            let cx = stock.x / 2.0;
            let y_start = margin;
            let y_end = stock.y - margin;
            if count == 1 {
                stock
                    .alignment_pins
                    .push(AlignmentPin::new(cx, stock.y / 2.0, diameter));
            } else {
                let step = (y_end - y_start) / (count - 1) as f64;
                for i in 0..count {
                    stock.alignment_pins.push(AlignmentPin::new(
                        cx,
                        y_start + step * i as f64,
                        diameter,
                    ));
                }
            }
        }
        None => {
            // No flip axis — distribute pins around the perimeter.
            // 2 pins: diagonal corners. 3+: spread along edges.
            if count <= 2 {
                stock
                    .alignment_pins
                    .push(AlignmentPin::new(margin, margin, diameter));
                if count == 2 {
                    stock.alignment_pins.push(AlignmentPin::new(
                        stock.x - margin,
                        stock.y - margin,
                        diameter,
                    ));
                }
            } else {
                // Place pins at evenly spaced positions around the perimeter
                let corners: &[[f64; 2]] = &[
                    [margin, margin],
                    [stock.x - margin, margin],
                    [stock.x - margin, stock.y - margin],
                    [margin, stock.y - margin],
                ];
                for i in 0..count {
                    let t = i as f64 / count as f64 * 4.0;
                    let seg = t.floor() as usize % 4;
                    let frac = t - seg as f64;
                    let [x0, y0] = corners[seg];
                    let [x1, y1] = corners[(seg + 1) % 4];
                    stock.alignment_pins.push(AlignmentPin::new(
                        x0 + (x1 - x0) * frac,
                        y0 + (y1 - y0) * frac,
                        diameter,
                    ));
                }
            }
        }
    }
}

/// Check if pins are symmetric about the flip axis (within tolerance).
fn pins_are_symmetric(pins: &[AlignmentPin], axis: FlipAxis, stock_x: f64, stock_y: f64) -> bool {
    const TOL: f64 = 0.5; // mm tolerance

    // For each pin, check that its mirror exists in the set
    for pin in pins {
        let m = mirror_pin(pin, axis, stock_x, stock_y);
        let has_mirror = pins
            .iter()
            .any(|p| (p.x - m.x).abs() < TOL && (p.y - m.y).abs() < TOL);
        if !has_mirror {
            return false;
        }
    }
    true
}

/// Hierarchical material picker: drills `Category ▶ → species`. Wood
/// nests one level deeper (`Wood ▶ → Softwood/Hardwood ▶ → species`).
/// Replaces the pre-2026-05-31 flat ComboBox + separate wood sub-picker
/// — the previous shape leaked the backend `Material` enum's flat
/// variant list into the UX as a 33+ entry dropdown that mixed all
/// classes together. UX complaint from the first bench smoke.
///
/// Backed by [`rs_cam_core::material::Material::materials_by_category`],
/// which merges the curated catalog with `WOOD_SPECIES_LIBRARY` and
/// dedups by Janka anchor. Per-leaf filter for the long Softwood /
/// Hardwood lists (each has ~60+ species after dedup).
///
/// Returns `true` if a selection was made this frame (caller pushes
/// `StockMaterialChanged`).
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
            let mut wood_buckets: Vec<&(MaterialCategory, Vec<(String, Material)>)> = groups
                .iter()
                .filter(|(c, _)| c.is_wood())
                .collect();
            wood_buckets.sort_by_key(|(c, _)| match c {
                MaterialCategory::Softwood => 0,
                _ => 1,
            });

            if !wood_buckets.is_empty() {
                ui.menu_button("Wood  ▶", |ui| {
                    for (cat, entries) in &wood_buckets {
                        if draw_wood_subcategory(ui, *cat, entries, stock) {
                            changed = true;
                            ui.close_menu();
                        }
                    }
                });
            }

            for (cat, entries) in &groups {
                if cat.is_wood() || entries.is_empty() {
                    continue;
                }
                ui.menu_button(format!("{}  ▶", cat.label()), |ui| {
                    for (label, mat) in entries {
                        let selected = stock.material == *mat;
                        if ui.selectable_label(selected, label).clicked() {
                            stock.material = mat.clone();
                            changed = true;
                            ui.close_menu();
                        }
                    }
                });
            }
        });
        response
            .response
            .on_hover_text(format!("Current: {} ({})", current_label, current_category.label()));
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
                .color(egui::Color32::from_rgb(140, 140, 150)),
        );
        ui.separator();

        egui::ScrollArea::vertical()
            .max_height(280.0)
            .show(ui, |ui| {
                for (entry_label, mat) in &visible {
                    let selected = stock.material == *mat;
                    if ui.selectable_label(selected, entry_label.as_str()).clicked() {
                        stock.material = mat.clone();
                        changed = true;
                    }
                }
                if visible.is_empty() {
                    ui.label(
                        egui::RichText::new("no matches")
                            .small()
                            .italics()
                            .color(egui::Color32::from_rgb(140, 140, 150)),
                    );
                }
            });
        ui.data_mut(|d| d.insert_temp(filter_id, filter));
    });
    changed
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn stock(x: f64, y: f64, padding: f64, pin_diameter: f64) -> StockConfig {
        let mut s = StockConfig {
            x,
            y,
            padding,
            ..StockConfig::default()
        };
        // Seed an existing pin so auto_place_pins picks up the diameter.
        s.alignment_pins
            .push(AlignmentPin::new(0.0, 0.0, pin_diameter));
        s
    }

    /// Auto-placed pins must keep their physical edge inside the stock,
    /// not just their centre. Pre-fix, padding=5 with a 6mm pin set
    /// margin=2.5 → pin edge clipped 0.5mm OUTSIDE the stock.
    #[test]
    fn auto_place_pins_keeps_pin_edge_inside_stock() {
        let mut s = stock(140.0, 150.0, 5.0, 6.0);
        auto_place_pins(&mut s, 2);
        for p in &s.alignment_pins {
            let r = p.diameter * 0.5;
            assert!(
                p.x - r >= -1e-6 && p.x + r <= s.x + 1e-6,
                "pin x={} r={} clips stock width {}",
                p.x,
                r,
                s.x,
            );
            assert!(
                p.y - r >= -1e-6 && p.y + r <= s.y + 1e-6,
                "pin y={} r={} clips stock height {}",
                p.y,
                r,
                s.y,
            );
        }
    }

    /// Edge-clearance: pin centre should sit at least
    /// `radius + EDGE_CLEARANCE (2mm)` from each edge so the pin body
    /// has 2mm of margin between the cutter side wall and the stock
    /// boundary.
    #[test]
    fn auto_place_pins_respects_edge_clearance() {
        let mut s = stock(140.0, 150.0, 5.0, 6.0);
        auto_place_pins(&mut s, 2);
        // Diameter 6 → radius 3; min centre offset from any edge = 5.0.
        for p in &s.alignment_pins {
            let edge_distance = p.x.min(s.x - p.x).min(p.y).min(s.y - p.y);
            assert!(
                edge_distance >= 5.0 - 1e-6,
                "pin at ({}, {}) sits {} from nearest edge; expected >= 5.0",
                p.x,
                p.y,
                edge_distance,
            );
        }
    }
}
