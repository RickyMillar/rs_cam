pub fn draw(ctx: &egui::Context, show: &mut bool) {
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
                ],
            );
            ui.add_space(8.0);

            // Overlays section (P6). Bound in every workspace that renders a
            // viewport, so it is its own block rather than a Toolpaths row.
            ui.heading("Overlays");
            draw_shortcut_grid(
                ui,
                &[
                    ("O", "Open / close the Overlays panel"),
                    ("Shift+O", "Pin / unpin it"),
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
    egui::Grid::new(ui.next_auto_id())
        .num_columns(2)
        .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
        .min_row_height(crate::ui::tokens::ROW_DENSE)
        .show(ui, |ui| {
            for (key, desc) in shortcuts {
                ui.label(egui::RichText::new(*key).strong().monospace());
                ui.label(*desc);
                ui.end_row();
            }
        });
}
