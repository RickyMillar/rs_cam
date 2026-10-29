use crate::ui::components::UiExt as _;
/// The shortcuts window. `scheme` is the mouse scheme of the settings
/// (B3): the "Viewport mouse" section prints its rows
/// (`interaction::input::rows`, the table `classify` reads).
pub fn draw(ctx: &egui::Context, show: &mut bool, scheme: rs_cam_core::settings::MouseScheme) {
    egui::Window::new("Keyboard Shortcuts")
        .open(show)
        .resizable(false)
        .default_width(380.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            // General section
            ui.heading("General");
            draw_shortcut_grid(
                ui,
                &[
                    ("Ctrl+O", "Open Job"),
                    ("Ctrl+S", "Save Job"),
                    ("Ctrl+Shift+E", "Export G-code (wizard)"),
                    ("Ctrl+Alt+E", "Direct export (skip wizard)"),
                    ("Ctrl+Z", "Undo"),
                    ("Ctrl+Shift+Z", "Redo"),
                    ("F12", "Screenshot"),
                ],
            );
            ui.add_space(8.0);

            // Toolpaths section
            ui.heading("Toolpaths");
            draw_shortcut_grid(
                ui,
                &[
                    ("G", "Generate selected"),
                    ("Shift+G", "Generate all"),
                    ("Delete", "Remove toolpath"),
                    ("I", "Toggle isolation"),
                    ("H", "Toggle visibility"),
                    ("Space", "Go to Simulation"),
                    ("1 / 2 / 3 / 4", "Top / Front / Right / Iso view"),
                    ("F", "Fit the selection (else the model)"),
                    ("Ctrl+A", "Pick mode: select every item of the chip kinds"),
                    ("Tab", "Pick mode: the next candidate under the pointer"),
                ],
            );
            ui.add_space(8.0);

            // B3: the viewport mouse. The drag rows are the rows `classify`
            // reads, so the text and the behaviour cannot differ.
            ui.heading("Viewport mouse");
            {
                use rs_cam_core::settings::SettingToken as _;
                ui.label(scheme.label());
            }
            let drags: Vec<(&str, &str)> = crate::interaction::input::rows(scheme)
                .iter()
                .map(|row| (row.keys, row.does))
                .collect();
            draw_shortcut_grid(ui, &drags);
            draw_shortcut_grid(
                ui,
                &[
                    ("Click", "Pick mode: add or remove the item"),
                    ("Shift+click", "Pick mode: add the item"),
                    ("Ctrl+click", "Pick mode: remove the item"),
                    ("Right-click", "Pick mode: the rule menu"),
                    ("Drag", "Pick mode: box (left to right encloses)"),
                    ("Wheel / pinch", "Zoom toward the pointer"),
                    ("F", "Fit the selection"),
                    ("Ctrl+A", "Pick mode: select every item of the chip kinds"),
                    ("Tab", "Pick mode: the next candidate under the pointer"),
                    (
                        "Escape",
                        "Cancel the box, then pause picking (Pick resumes)",
                    ),
                ],
            );
            ui.add_space(8.0);

            // Viewport options (redesign phase 2). Bound in every workspace
            // that renders a viewport, so it is its own block rather than a
            // Toolpaths row.
            ui.heading("Viewport options");
            draw_shortcut_grid(
                ui,
                &[
                    ("O", "Open / close All viewport options"),
                    ("S", "Stock box"),
                    ("P", "Cutting paths"),
                    ("R", "Rapids"),
                    ("X", "Collisions"),
                    (", / .", "Step the model / stock colour source"),
                ],
            );
            ui.add_space(8.0);

            // Simulation section
            ui.heading("Simulation");
            draw_shortcut_grid(
                ui,
                &[
                    ("Space", "Play / Pause"),
                    ("Left / Right", "Step backward / forward"),
                    ("Home / End", "Jump to start / end"),
                    ("[ / ]", "Decrease / Increase speed"),
                    ("Escape", "Back to Toolpaths"),
                ],
            );
        });
}

fn draw_shortcut_grid(ui: &mut egui::Ui, shortcuts: &[(&str, &str)]) {
    ui.param_grid(ui.next_auto_id(), |ui| {
        for (key, desc) in shortcuts {
            ui.label(egui::RichText::new(*key).strong().monospace());
            ui.label(*desc);
            ui.end_row();
        }
    });
}
