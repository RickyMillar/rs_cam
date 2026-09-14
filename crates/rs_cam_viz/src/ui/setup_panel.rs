//! The Setup workspace's left rail: the project's resources, then its setups.
//!
//! # What DC4 changed and why
//!
//! `planning/ui_declutter_2026-09-14/PLAN.md` Pattern D: the same kind of
//! object was drawn at three weights. Stock was a full card. Machine was a
//! full card. Models was a `▸` disclosure. Tools was a `▸` disclosure in a
//! different workspace. The two setup cards were different HEIGHTS from each
//! other, because one carried an extra line.
//!
//! Stock, Machine, Models and Tools are the project's RESOURCES. You click
//! one to open its editor in the right sidebar, so they are NAVIGATION, not
//! content, and a navigation item is one line. Each is now one
//! [`tokens::ROW_DENSE`] row: a label, its current value, and a chevron.
//!
//! Ruling R29 brought the Tool Library here from the Toolpaths workspace. A
//! tool is a resource, so it joins the resource list rather than taking a
//! fifth workspace tab.
//!
//! # Rule D: one object kind, one weight, one height
//!
//! A setup card is ONE row at a fixed height. The name truncates and the
//! orientation is one short word off a fixed list, so no card can grow a
//! second line. The lines that used to vary between cards — the datum, the
//! workholding counts, the flip instruction and the fresh-stock note — are
//! on the card's hover text, and the setup's own properties panel edits
//! them.

use super::AppEvent;
use crate::state::AppState;
use crate::state::job::{FaceUp, ModelId, SetupId};
use crate::state::selection::Selection;
use crate::ui::components::{Card, Role};
use crate::ui::tokens;
use crate::ui_command::{NoArgs, UiCommand};
use rs_cam_core::session::SetupData;
use rs_cam_core::session::XYDatum;

/// The chevron that marks a row as navigation.
const CHEVRON: &str = "\u{203A}";

/// The overflow menu's glyph.
const ELLIPSIS: &str = "\u{2026}";

/// The label column of a resource row.
///
/// Every resource row shares one left edge for its value, which is what
/// makes the four rows read as one list. 64 points holds the longest label,
/// `Machine`, at the body rung with room to spare, and stays on the grid.
/// A column narrower than the longest label lets that one row push its
/// value right and the shared edge is then gone.
const LABEL_COLUMN: f32 = tokens::SPACE_7 + tokens::SPACE_7;

/// The state dot. Ruling R30 sets a 6-point indicator, so the diameter
/// comes off the grid rather than from a literal.
const DOT_DIAMETER: f32 = tokens::SPACE_3 - tokens::SPACE_1;

/// Half the diameter, which is what the painter asks for.
const DOT_RADIUS: f32 = DOT_DIAMETER / 2.0;

/// The width the state dot reserves in a row.
const DOT_SLOT: f32 = tokens::SPACE_3;

/// Left panel for the Setup workspace: the resource rows, then the setups.
pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.add_space(tokens::SPACE_2);

    draw_stock_row(ui, state, events);
    draw_machine_row(ui, state, events);
    draw_models_row(ui, state, events);
    draw_tools_row(ui, state, events);

    // The rows above and the cards below are different KINDS of object, so a
    // hairline separates them. Every row above is navigation. Every card
    // below is a setup you work in.
    ui.add_space(tokens::SPACE_3);
    let y = ui.cursor().top();
    let x = ui.max_rect().x_range();
    ui.painter()
        .hline(x, y, egui::Stroke::new(1.0, tokens::HAIRLINE));
    ui.add_space(tokens::SPACE_3);

    for setup in state.session.list_setups() {
        draw_setup_card(ui, setup, state, events);
        ui.add_space(tokens::SPACE_2);
    }

    ui.add_space(tokens::SPACE_2);
    if ui.button("+").on_hover_text("Add a setup").clicked() {
        events.push(AppEvent::AddSetup);
    }
}

// ── the resource rows ────────────────────────────────────────────────

/// The Stock row. The value is the EFFECTIVE size for the active setup's
/// orientation, so it accounts for `face_up` and `z_rotation`.
fn draw_stock_row(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    let stock = state.session.stock_config();
    let (eff_w, eff_d, eff_h) = if let Some(setup) = active_setup(state) {
        let (w, d, h) = setup.face_up.effective_stock(stock.x, stock.y, stock.z);
        setup.z_rotation.effective_stock(w, d, h)
    } else {
        (stock.x, stock.y, stock.z)
    };
    let value = format!("{eff_w:.0} × {eff_d:.0} × {eff_h:.0} mm");

    let selected = state.selection == Selection::Stock;
    let row = resource_row(ui, "Stock", &value, selected, None);
    if row.on_hover_text("Stock dimensions and material").clicked() {
        events.push(AppEvent::Ui(UiCommand::Select(Selection::Stock)));
    }
}

/// The Machine row. It is the only entry point to the Machine Setup panel —
/// preset, feeds, kinematics and the GRBL `$$` import.
fn draw_machine_row(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    let name = state.session.machine().name.as_str();
    let selected = state.selection == Selection::Machine;
    let row = resource_row(ui, "Machine", name, selected, None);
    if row.on_hover_text("Machine, feeds and kinematics").clicked() {
        events.push(AppEvent::Ui(UiCommand::Select(Selection::Machine)));
    }
}

/// The Models row, with its list nested under it while a model is selected.
///
/// Selection IS the disclosure state. The row therefore needs no stored
/// open flag, and opening the list and opening the editor are one click.
fn draw_models_row(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    let models = state.session.models();
    let open = matches!(
        state.selection,
        Selection::Model(_) | Selection::Face(..) | Selection::Faces(..)
    );

    let row = resource_row(ui, "Models", &models.len().to_string(), open, None);
    if row.on_hover_text("The project's models").clicked() {
        // A second click on an open row closes it, which is what a
        // disclosure does.
        if open {
            events.push(AppEvent::Ui(UiCommand::Select(Selection::None)));
        } else if let Some(first) = models.first() {
            events.push(AppEvent::Ui(UiCommand::Select(Selection::Model(ModelId(
                first.id,
            )))));
        }
    }

    if !open {
        return;
    }
    ui.indent("setup_rail_models", |ui| {
        for model in models {
            let mid = ModelId(model.id);
            let selected = state.selection == Selection::Model(mid);
            let response = ui.selectable_label(selected, &model.name);
            if response.clicked() {
                events.push(AppEvent::Ui(UiCommand::Select(Selection::Model(mid))));
            }
            response.context_menu(|ui| {
                if ui.button("Reload from disk").clicked() {
                    events.push(AppEvent::ReloadModel(mid));
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    events.push(AppEvent::RemoveModel(mid));
                    ui.close();
                }
            });
        }
    });
}

/// The Tools row — the Tool Library's new home (ruling R29).
///
/// It carries the full CRUD set the Toolpaths workspace used to hold:
/// Duplicate and Delete on each tool, the library manager, and the two add
/// routes. The row's `…` menu holds the actions; the nested list holds the
/// tools and appears while a tool is selected.
fn draw_tools_row(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    let open = matches!(state.selection, Selection::Tool(_));
    let count = state.session.tools().len();

    let mut menu = |ui: &mut egui::Ui| {
        if ui
            .button("Manage library…")
            .on_hover_text("Browse, edit, and organise the reusable tool catalogs.")
            .clicked()
        {
            events.push(AppEvent::Ui(UiCommand::OpenToolLibrary(NoArgs)));
            ui.close();
        }
        ui.separator();
        ui.menu_button("Add tool", |ui| {
            for &tt in crate::state::job::ToolType::ALL {
                if ui.button(tt.label()).clicked() {
                    events.push(AppEvent::AddTool(tt));
                    ui.close();
                }
            }
            let libraries = rs_cam_core::tool_library::list_libraries();
            if libraries.is_empty() {
                return;
            }
            ui.separator();
            ui.menu_button("From library", |ui| {
                for lib in &libraries {
                    ui.menu_button(lib, |ui| {
                        match rs_cam_core::tool_library::load_library(lib) {
                            Ok(catalog) if catalog.tools.is_empty() => {
                                ui.label("(empty)");
                            }
                            Ok(catalog) => {
                                for tool in &catalog.tools {
                                    let label = format!("{} — ⌀{:.2}mm", tool.name, tool.diameter);
                                    if ui.button(label).clicked() {
                                        events.push(AppEvent::AddToolFromLibrary(Box::new(
                                            tool.clone(),
                                        )));
                                        ui.close();
                                    }
                                }
                            }
                            Err(e) => {
                                ui.label(format!("load error: {e}"));
                            }
                        }
                    });
                }
            });
        });
    };

    let row = resource_row(ui, "Tools", &count.to_string(), open, Some(&mut menu));
    if row.on_hover_text("The tool library").clicked() {
        if open {
            events.push(AppEvent::Ui(UiCommand::Select(Selection::None)));
        } else if let Some(first) = state.session.tools().first() {
            events.push(AppEvent::Ui(UiCommand::Select(Selection::Tool(first.id))));
        }
    }

    if !open {
        return;
    }
    ui.indent("setup_rail_tools", |ui| {
        for tool in state.session.tools() {
            let selected = state.selection == Selection::Tool(tool.id);
            let response = ui.selectable_label(selected, tool.summary());
            if response.clicked() {
                events.push(AppEvent::Ui(UiCommand::Select(Selection::Tool(tool.id))));
            }
            response.context_menu(|ui| {
                if ui.button("Duplicate").clicked() {
                    events.push(AppEvent::DuplicateTool(tool.id));
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    events.push(AppEvent::RemoveTool(tool.id));
                    ui.close();
                }
            });
        }
    });
}

/// One resource row: `label · value · […] · ›`, at [`tokens::ROW_DENSE`].
///
/// The row carries NO card frame. A card says "this is an object you read";
/// a resource row says "this opens somewhere else".
///
/// The row's own click sense is registered through `UiBuilder::sense`, which
/// egui places BELOW the senses of the widgets inside it. The `…` menu
/// therefore receives its own clicks and the row receives the rest.
fn resource_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    selected: bool,
    menu: Option<&mut dyn FnMut(&mut egui::Ui)>,
) -> egui::Response {
    ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
        let fill = if selected {
            tokens::ACCENT_QUIET
        } else if ui.response().hovered() {
            tokens::hover_lift(tokens::SURFACE_BASE)
        } else {
            tokens::SURFACE_BASE
        };
        egui::Frame::default()
            .fill(fill)
            .corner_radius(tokens::RADIUS_SM)
            .inner_margin(egui::Margin::symmetric(tokens::SPACE_2 as i8, 0))
            .show(ui, |ui| {
                let width = ui.available_width();
                ui.set_min_width(width);
                ui.spacing_mut().interact_size.y = tokens::ROW_DENSE;
                ui.horizontal(|ui| {
                    ui.set_min_height(tokens::ROW_DENSE);
                    ui.allocate_ui(egui::vec2(LABEL_COLUMN, tokens::ROW_DENSE), |ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(label).color(tokens::TEXT_MUTED))
                                .selectable(false),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(CHEVRON).color(tokens::TEXT_FAINT));
                        if let Some(menu) = menu {
                            ui.menu_button(ELLIPSIS, menu);
                        }
                        // The value reads from the label column
                        // rightwards and truncates, so a long
                        // machine name cannot grow the row.
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(value).color(tokens::TEXT_STRONG),
                                )
                                .truncate()
                                .selectable(false),
                            );
                        });
                    });
                });
            });
    })
    .response
}

// ── the setup cards ──────────────────────────────────────────────────

/// One setup card: `dot · name · orientation`, at ONE fixed height.
///
/// The dot is the setup's generation state, the figure DC2 removed from the
/// Toolpaths panel's per-setup header. The operator ruled that the `2/3`
/// fraction "means nothing obvious" and asked for indicators on cards, so
/// the count is on hover and the card carries the Role.
fn draw_setup_card(
    ui: &mut egui::Ui,
    setup: &SetupData,
    state: &AppState,
    events: &mut Vec<AppEvent>,
) {
    let setup_id = SetupId(setup.id);
    let is_selected = state.selection == Selection::Setup(setup_id);

    // F2.2 — `is_current()`, not `ComputeStatus::Done`. An edited operation
    // keeps `Done`, so a header that read `n/n` disagreed with a card that
    // said STALE about the same operation.
    let total = setup.toolpath_indices.len();
    let ready = setup
        .toolpath_indices
        .iter()
        .filter(|&&idx| {
            crate::state::freshness::freshness_at(&state.session, &state.gui, idx)
                .is_some_and(|f| f.is_current())
        })
        .count();
    let (role, state_line) = match (total, ready) {
        (0, _) => (Role::Unknown, "No operations".to_owned()),
        (t, 0) => (Role::Unknown, format!("0 of {t} operations current")),
        (t, r) if r == t => (Role::Ok, format!("All {t} operations current")),
        (t, r) => (Role::Caution, format!("{r} of {t} operations current")),
    };

    let orient = if setup.z_rotation == rs_cam_core::compute::transform::ZRotation::Deg0 {
        setup.face_up.label().to_owned()
    } else {
        format!("{} +{}", setup.face_up.label(), setup.z_rotation.label())
    };

    let detail = setup_detail(setup, state, &state_line, &orient);

    let response = ui
        .scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
            let hovered = ui.response().hovered();
            Card::new()
                .selected(is_selected)
                .hovered(hovered)
                .show(ui, |ui| {
                    let width = ui.available_width();
                    ui.set_min_width(width);
                    ui.horizontal(|ui| {
                        ui.set_min_height(tokens::ROW_DENSE);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&orient)
                                        .small()
                                        .color(tokens::TEXT_MUTED),
                                )
                                .selectable(false),
                            );
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    state_dot(ui, role);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&setup.name)
                                                .color(tokens::TEXT_STRONG),
                                        )
                                        .truncate()
                                        .selectable(false),
                                    );
                                },
                            );
                        });
                    });
                });
        })
        .response;

    if response.on_hover_text(detail).clicked() {
        events.push(AppEvent::Ui(UiCommand::Select(Selection::Setup(setup_id))));
    }
}

/// The lines that used to sit on the card and made one card taller than
/// another. They are hover text now (Rule D).
fn setup_detail(setup: &SetupData, state: &AppState, state_line: &str, orient: &str) -> String {
    let datum = match &setup.datum.xy_method {
        XYDatum::CornerProbe(c) => format!("Corner ({})", c.label()),
        XYDatum::CenterOfStock => "Center".to_owned(),
        XYDatum::AlignmentPins => "Pins".to_owned(),
        XYDatum::Manual => "Manual".to_owned(),
    };
    let fixtures = setup.fixtures.len();
    let keep_out = setup.keep_out_zones.len();
    let pins = state.session.stock_config().alignment_pins.len();

    let mut detail = format!("{state_line}\nOrientation: {orient}\nXY datum: {datum}");
    if fixtures == 0 && keep_out == 0 && pins == 0 {
        detail.push_str("\nNo workholding");
    } else {
        detail.push_str(&format!(
            "\nFixtures {fixtures} · Keep-out {keep_out} · Pins {pins}"
        ));
    }
    if setup.face_up != FaceUp::Top {
        detail.push('\n');
        detail.push_str(setup.face_up.flip_instruction());
    }
    // The fresh-stock note for every setup after the first. A full italic
    // line on every Setup-2+ card forever was warning-as-wallpaper.
    if state.session.list_setups().first().map(|s| s.id) != Some(setup.id) {
        detail.push_str(
            "\nThis setup's simulation starts from uncut stock — material removed \
             by prior setups is not reflected.",
        );
    }
    detail
}

/// Paint the state dot.
///
/// §2.6 rule 3 says colour is never the only channel, so every caller also
/// writes the same state as words into the row's hover text.
fn state_dot(ui: &mut egui::Ui, role: Role) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(DOT_SLOT, tokens::ROW_DENSE),
        egui::Sense::hover(),
    );
    ui.painter()
        .circle_filled(rect.center(), DOT_RADIUS, role.text());
}

/// Determine the active setup from the current selection.
fn active_setup(state: &AppState) -> Option<&SetupData> {
    let setups = state.session.list_setups();
    let setup_id = match &state.selection {
        Selection::Setup(id) => Some(*id),
        Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
        Selection::Toolpath(tp_id) => state
            .session
            .setup_of_toolpath_id(*tp_id)
            .and_then(|idx| setups.get(idx))
            .map(|s| SetupId(s.id)),
        _ => None,
    };
    if let Some(sid) = setup_id {
        setups.iter().find(|s| s.id == sid.0)
    } else {
        setups.first()
    }
}
