//! G-VPDOCK: the viewport dock and the All viewport options catalogue reach
//! every registry row (viewport redesign, phase 2).
//!
//! The dock replaced the strip above the 3D view, and the catalogue replaced
//! the Overlays panel. These arms hold the promises the redesign made:
//!
//! - (a) every registry row has exactly one dock section, and the catalogue
//!   walks every section, so every row is reachable;
//! - (b) at most one popover is open, and `Escape` closes one surface at a
//!   time;
//! - (c) the dock constructs the three commands the strip alone constructed;
//! - (d) `O` opens the catalogue and `Shift+O` is bound nowhere;
//! - (e) at a 320 pt viewport, the dock and each popover stay inside the
//!   viewport width.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::Path;

use rs_cam_viz::compute::{ComputeLane, LaneSnapshot, LaneState};
use rs_cam_viz::render::camera::ProjectionMode;
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::overlays::{DockSection, OverlayPanelState};
use rs_cam_viz::ui::overlays::registry;
use rs_cam_viz::ui::viewport_overlay::{self, DOCK_AREA_ID, DockLayout, POPOVER_AREA_ID};
use rs_cam_viz::ui::{AppEvent, tokens};

// ── fixtures ─────────────────────────────────────────────────────────

fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(
        text.len() > 500,
        "{} is too short to be the file this scan means",
        path.display()
    );
    text
}

/// The source with every `//` comment removed, so a scan never matches the
/// prose that explains a deletion.
fn code_only(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── (a) every row is reachable ───────────────────────────────────────

/// MOCKUPS §1 gives each of the 39 rows ONE dock home. The counts are the
/// table's; a new row lands in the section of its group.
#[test]
fn every_registry_row_has_one_dock_home_g_vpdock() {
    let rows = registry::rows();
    assert!(
        rows.len() >= 39,
        "the registry lists only {} rows; this arm reads the wrong list",
        rows.len()
    );

    let mut homes: BTreeMap<&str, Vec<DockSection>> = BTreeMap::new();
    for section in DockSection::ALL {
        for row in registry::rows_in_section(section) {
            homes.entry(row.id).or_default().push(section);
        }
    }
    for row in rows {
        let found = homes.get(row.id).cloned().unwrap_or_default();
        assert_eq!(
            found,
            vec![registry::dock_section(row)],
            "row `{}` must have exactly one dock home; it is listed under {found:?}",
            row.id
        );
    }
    assert_eq!(
        homes.len(),
        rows.len(),
        "the dock sections do not cover the registry"
    );

    // The MOCKUPS §1 table, as it stood when the dock was built.
    let count = |section| registry::rows_in_section(section).count();
    assert_eq!(
        count(DockSection::View),
        1,
        "View holds the orientation gizmo"
    );
    assert_eq!(count(DockSection::Scene), 14, "Scene holds 14 rows");
    assert_eq!(count(DockSection::Paths), 13, "Paths holds 13 rows");
    // 12 since the By Area regions row (2026-09-24) joined Inspect.
    assert_eq!(count(DockSection::Inspect), 12, "Inspect holds 12 rows");
    for (id, section) in [
        ("orientation_gizmo", DockSection::View),
        ("simulated_stock", DockSection::Scene),
        ("boundary_outline", DockSection::Scene),
        ("move_colour_engagement", DockSection::Paths),
        ("all_toolpaths", DockSection::Paths),
        ("reach_map", DockSection::Inspect),
        ("area_regions", DockSection::Inspect),
        ("stock_colour_deviation", DockSection::Inspect),
    ] {
        let row = registry::row(id).unwrap_or_else(|| panic!("row `{id}` is gone"));
        assert_eq!(registry::dock_section(row), section, "`{id}` moved section");
    }
}

/// The catalogue walks every section with the registry's own section list,
/// and each popover draws the rows it does not place itself. So a row can
/// never exist in the registry and be missing from both surfaces.
#[test]
fn the_catalogue_and_the_popovers_walk_the_section_lists_g_vpdock() {
    let catalogue = code_only(&source("src/ui/overlays/panel.rs"));
    assert!(
        catalogue.contains("for section in DockSection::ALL")
            && catalogue.contains("registry::rows_in_section(section)"),
        "the catalogue no longer lists every section's rows"
    );
    assert!(
        !catalogue.contains("RunSimulation"),
        "the catalogue restores a Run Simulation route"
    );

    let dock = code_only(&source("src/ui/viewport_overlay.rs"));
    for section in ["View", "Paths", "Inspect"] {
        let call = format!("draw_remaining_rows(ui, state, events, DockSection::{section}");
        assert!(
            dock.contains(&call),
            "the {section} popover no longer draws the rows it does not place itself"
        );
    }
    assert!(
        dock.contains("for row in registry::rows_in_section(DockSection::Scene)"),
        "the Scene popover no longer walks its section list"
    );
}

// ── (b) one popover at a time ────────────────────────────────────────

#[test]
fn at_most_one_popover_is_open_g_vpdock() {
    let mut overlays = OverlayPanelState::new();
    assert_eq!(overlays.open_section, None, "the dock opens closed");

    overlays.toggle_section(DockSection::Scene);
    assert_eq!(overlays.open_section, Some(DockSection::Scene));

    // A second section closes the first and opens the second.
    overlays.toggle_section(DockSection::Inspect);
    assert_eq!(overlays.open_section, Some(DockSection::Inspect));

    // The same section again closes it.
    overlays.toggle_section(DockSection::Inspect);
    assert_eq!(overlays.open_section, None);

    // Escape closes the popover first, then the catalogue, then nothing.
    overlays.open = true;
    overlays.toggle_section(DockSection::Paths);
    assert!(overlays.close_one_for_escape());
    assert_eq!(overlays.open_section, None);
    assert!(overlays.open, "the first Escape closes the popover only");
    assert!(overlays.close_one_for_escape());
    assert!(!overlays.open, "the second Escape closes the catalogue");
    assert!(
        !overlays.close_one_for_escape(),
        "with nothing open, Escape keeps its workspace meaning"
    );
}

/// The keyboard guard runs before the Simulation Escape arm, so an open
/// popover never also switches the workspace.
#[test]
fn escape_is_consumed_before_the_workspace_handler_g_vpdock() {
    let input = code_only(&source("src/app/input.rs"));
    let guard = "self.escape_closes_a_viewport_surface(ctx);";
    let sim_start = input
        .find("fn handle_simulation_shortcuts(")
        .expect("the Simulation shortcut handler is gone");
    let sim = &input[sim_start..];
    let guard_at = sim
        .find(guard)
        .expect("the Simulation handler lost the guard");
    let escape_at = sim
        .find("egui::Key::Escape")
        .expect("the Simulation Escape arm is gone");
    assert!(
        guard_at < escape_at,
        "the popover guard must run before the Simulation Escape arm"
    );
    let main_start = input
        .find("fn handle_keyboard_shortcuts(")
        .expect("the main shortcut handler is gone");
    assert!(
        input[main_start..].contains(guard),
        "the main handler lost the guard"
    );
    assert!(
        input.contains("consume_key(egui::Modifiers::NONE, egui::Key::Escape)"),
        "the guard no longer consumes the key"
    );
}

// ── (c) the three commands ───────────────────────────────────────────

/// `ToggleProjection`, `ToggleShowAllToolpaths` and `CancelCompute` had the
/// strip as their ONLY GUI constructor. The dock took each one over in the
/// same change (inventory §1.8, §5.4).
#[test]
fn the_dock_constructs_the_three_strip_commands_g_vpdock() {
    let dock = code_only(&source("src/ui/viewport_overlay.rs"));
    assert!(
        dock.contains("pub fn draw(") && dock.contains("DockSection::ALL"),
        "non-vacuity: the dock file no longer holds the dock"
    );
    for command in [
        "ToggleProjection",
        "ToggleShowAllToolpaths",
        "CancelCompute",
    ] {
        let needle = format!("AppEvent::Ui(UiCommand::{command}(NoArgs))");
        assert!(
            dock.contains(&needle),
            "the dock no longer constructs UiCommand::{command}; its row in \
             ui_command.rs claims a GUI reach that nothing makes"
        );
    }
    assert!(
        dock.contains("\"overlay_cancel_all\"") && dock.contains("\"overlay_collision_check\""),
        "the two automation ids the smoke harness reads left the dock"
    );
}

// ── (d) the keys ─────────────────────────────────────────────────────

#[test]
fn o_opens_the_catalogue_and_shift_o_is_retired_g_vpdock() {
    let input = code_only(&source("src/app/input.rs"));
    let arms: Vec<&str> = input
        .lines()
        .filter(|line| line.contains("egui::Key::O)"))
        .collect();
    assert_eq!(
        arms.len(),
        1,
        "input.rs must bind the bare `O` once; found {arms:?}"
    );
    assert!(
        arms[0].contains("!modifiers.shift"),
        "the `O` arm must refuse Shift, or Shift+O keeps a meaning: {}",
        arms[0]
    );
    let at = input.find(arms[0]).unwrap();
    let body: String = input[at..].lines().take(4).collect::<Vec<_>>().join("\n");
    assert!(
        body.contains("overlays.open = !overlays.open"),
        "`O` no longer toggles the catalogue:\n{body}"
    );
    assert!(
        !input.contains("pinned"),
        "input.rs still writes a pinned panel; Shift+O pinned the retired column"
    );

    // Nowhere else binds a bare or shifted O. The menu's Ctrl+O opens a
    // project and refuses Shift.
    let menu = code_only(&source("src/ui/menu_bar.rs"));
    for line in menu.lines().filter(|line| line.contains("Key::O)")) {
        assert!(
            line.contains("modifiers.ctrl") && line.contains("!modifiers.shift"),
            "menu_bar.rs binds O without Ctrl, or with Shift: {line}"
        );
    }
    let state = code_only(&source("src/state/overlays.rs"));
    assert!(
        !state.contains("pub pinned"),
        "the pinned field is back; nothing may pin a column that does not exist"
    );
}

// ── (e) the dock fits a 320 pt viewport ──────────────────────────────

fn ctx(width: f32, height: f32) -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(raw_input(width, height), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

fn raw_input(width: f32, height: f32) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, height),
        )),
        ..Default::default()
    }
}

/// A lane set with one running job, so the dock also draws its compute
/// activity and the `Cancel all jobs` button: the widest dock.
fn busy_lanes() -> [LaneSnapshot; 5] {
    let mut toolpath = LaneSnapshot::idle(ComputeLane::Toolpath);
    toolpath.state = LaneState::Running;
    toolpath.current_job = Some("Adaptive 3D".to_owned());
    [
        toolpath,
        LaneSnapshot::idle(ComputeLane::Analysis),
        LaneSnapshot::idle(ComputeLane::Optimize),
        LaneSnapshot::idle(ComputeLane::Job),
        LaneSnapshot::idle(ComputeLane::Reach),
    ]
}

/// Run a few passes (an `Area` sizes itself on its first pass) and give
/// back the rect of each area.
fn render_dock(
    state: &mut AppState,
    viewport: egui::Rect,
) -> (Option<egui::Rect>, Option<egui::Rect>) {
    let ctx = ctx(viewport.width(), viewport.height());
    let lanes = busy_lanes();
    let mut events: Vec<AppEvent> = Vec::new();
    for _ in 0..3 {
        let mut output = ctx.run_ui(raw_input(viewport.width(), viewport.height()), |ui| {
            viewport_overlay::draw(
                ui,
                state,
                ProjectionMode::Perspective,
                &lanes,
                &mut events,
                viewport,
            );
        });
        output.textures_delta.clear();
    }
    ctx.memory(|m| {
        (
            m.area_rect(egui::Id::new(DOCK_AREA_ID)),
            m.area_rect(egui::Id::new(POPOVER_AREA_ID)),
        )
    })
}

fn assert_inside(name: &str, rect: egui::Rect, viewport: egui::Rect) {
    assert!(
        rect.left() >= viewport.left() - 0.5 && rect.right() <= viewport.right() + 0.5,
        "{name} overflows a {} pt viewport horizontally: {rect:?}",
        viewport.width()
    );
    assert!(rect.width() > 1.0, "{name} drew nothing: {rect:?}");
}

#[test]
fn the_dock_fits_a_320_pt_viewport_g_vpdock() {
    let viewport = egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(rs_cam_viz::ui::overlays::panel::MIN_VIEWPORT_WIDTH, 600.0),
    );

    // MOCKUPS §7: at 320 pt the suffixes hide and the catalogue button is
    // the glyph alone. The four sections never hide.
    let layout = DockLayout::for_viewport_width(viewport.width());
    assert!(!layout.show_suffixes, "320 pt is under the suffix limit");
    assert!(
        !layout.labelled_catalogue,
        "288 pt available is under the labelled catalogue limit"
    );
    let wide = DockLayout::for_viewport_width(1000.0);
    assert!(wide.show_suffixes && wide.labelled_catalogue);

    let mut state = AppState::new();
    let (dock, popover) = render_dock(&mut state, viewport);
    let dock = dock.expect("the dock area never drew");
    assert_inside("the dock", dock, viewport);
    assert!(popover.is_none(), "a popover drew with no section open");

    for section in DockSection::ALL {
        let mut state = AppState::new();
        state.overlays.open_section = Some(section);
        let (dock, popover) = render_dock(&mut state, viewport);
        assert_inside(
            "the dock",
            dock.expect("the dock area never drew"),
            viewport,
        );
        let popover = popover.unwrap_or_else(|| panic!("the {section:?} popover never drew"));
        assert_inside(&format!("the {section:?} popover"), popover, viewport);
    }
}
