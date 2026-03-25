pub fn draw(ctx: &egui::Context, show: &mut bool) {
    egui::Window::new("Keyboard Shortcuts")
        .open(show)
        .resizable(false)
        .default_width(380.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            // General section
            ui.heading("General");
            draw_shortcut_grid(ui, &[
                ("Ctrl+O", "Open Job"),
                ("Ctrl+S", "Save Job"),
                ("Ctrl+Shift+E", "Export G-code"),
                ("Ctrl+Z", "Undo"),
                ("Ctrl+Shift+Z", "Redo"),
                ("F12", "Screenshot"),
            ]);
            ui.add_space(8.0);

            // Toolpaths section
            ui.heading("Toolpaths");
            draw_shortcut_grid(ui, &[
                ("G", "Generate selected"),
                ("Shift+G", "Generate all"),
                ("Delete", "Remove toolpath"),
                ("I", "Toggle isolation"),
                ("H", "Toggle visibility"),
                ("Space", "Go to Simulation"),
                ("1 / 2 / 3 / 4", "Top / Front / Right / Iso view"),
            ]);
            ui.add_space(8.0);

            // Simulation section
            ui.heading("Simulation");
            draw_shortcut_grid(ui, &[
                ("Space", "Play / Pause"),
                ("Left / Right", "Step backward / forward"),
                ("Home / End", "Jump to start / end"),
                ("[ / ]", "Decrease / Increase speed"),
                ("Escape", "Back to Toolpaths"),
            ]);
        });
}

fn draw_shortcut_grid(ui: &mut egui::Ui, shortcuts: &[(&str, &str)]) {
    egui::Grid::new(ui.next_auto_id())
        .num_columns(2)
        .spacing([16.0, 4.0])
        .show(ui, |ui| {
            for (key, desc) in shortcuts {
                ui.label(egui::RichText::new(*key).strong().monospace());
                ui.label(*desc);
                ui.end_row();
            }
        });
}
