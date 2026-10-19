//! F218's sentry: a native file dialog never blocks the frame.
//!
//! A blocking `rfd::FileDialog` used to run inside the draw at 18 sites.
//! While it was open the frame stopped, and the MCP drain, which runs once
//! per frame, stopped with it. `app/file_dialog.rs` is now the one door: it
//! runs the dialog on its own thread and posts the pick as an `AppEvent`.
//!
//! Arm 1 refuses an `rfd::FileDialog::new()` (and the other `rfd` dialog
//! types) in any `src/` file other than the helper. Arm 2 refuses a file
//! read in the Machine panel draw: the GRBL `$$` file is read after the
//! pick. Arm 3 is the non-vacuity anchor: the helper holds the one
//! construction and the scan reads the files it names.
//!
//! The behaviour arms (one dialog at a time, a pick posted once, a cancel
//! frees the slot) are unit tests beside the type, in `app/file_dialog.rs`.
//!
//! NOT MEASURED: that the dialog shows its title, filters and default name
//! on screen. The request type carries each option, and a unit test reads
//! them back.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The one file that may build an `rfd` dialog.
const HELPER: &str = "app/file_dialog.rs";

/// The `rfd` dialog spellings that block or that a draw could block on.
const DIALOG_NEEDLES: &[&str] = &[
    "rfd::FileDialog",
    "rfd::AsyncFileDialog",
    "FileDialog::new(",
];

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The source with every `//` comment cut, so a doc line that names the
/// old call does not match.
fn code_only(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn relative(path: &Path) -> String {
    path.strip_prefix(src_root())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

fn read_code(relative: &str) -> String {
    let path = src_root().join(relative);
    code_only(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display())),
    )
}

#[test]
fn no_file_dialog_is_built_outside_the_helper_f218() {
    let mut files = Vec::new();
    rust_files(&src_root(), &mut files);
    assert!(
        files.len() > 100,
        "the scan found only {} source files; it reads the wrong folder",
        files.len()
    );
    let mut offenders = Vec::new();
    for path in &files {
        let name = relative(path);
        if name == HELPER {
            continue;
        }
        let code = code_only(&std::fs::read_to_string(path).unwrap());
        for needle in DIALOG_NEEDLES {
            if code.contains(needle) {
                offenders.push(format!("{name}: `{needle}`"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a file dialog built outside `{HELPER}` blocks the frame and the MCP \
         drain while it is open. Build a `FileDialogRequest` and push \
         `request.event(AfterPick::..)` (a draw) or call \
         `open_file_dialog` (app code): {offenders:#?}"
    );
}

#[test]
fn the_machine_panel_reads_no_file_in_its_draw_f218() {
    let panel = read_code("ui/properties/machine_panel.rs");
    assert!(
        panel.contains("AfterPick::LoadGrblSettings"),
        "the anchor moved: the Machine panel no longer asks for the GRBL file"
    );
    for needle in ["read_to_string", "std::fs::"] {
        assert!(
            !panel.contains(needle),
            "the Machine panel draw holds `{needle}`; read the file after the \
             pick (`AfterPick::LoadGrblSettings` in `{HELPER}`)"
        );
    }
    assert!(
        read_code(HELPER).contains("std::fs::read_to_string(&path)"),
        "the GRBL file read lives in the pick continuation of `{HELPER}`"
    );
}

#[test]
fn the_helper_holds_the_one_dialog_construction_f218() {
    let helper = read_code(HELPER);
    assert_eq!(
        helper.matches("rfd::FileDialog::new()").count(),
        1,
        "`{HELPER}` builds the dialog in exactly one place (`FileDialogRequest::run`)"
    );
    assert!(
        helper.contains("std::thread::Builder::new()"),
        "the dialog runs on its own thread, not in the frame"
    );
    assert!(
        helper.contains("ctx.request_repaint()"),
        "the dialog thread wakes the frame, or a parked frame waits for input"
    );
    // The scan must see the sites it once caught: each now names the helper.
    for site in [
        "ui/menu_bar.rs",
        "ui/setup_panel.rs",
        "ui/properties/model_sim_panels.rs",
        "ui/properties/machine_panel.rs",
        "app/input.rs",
        "app/export.rs",
    ] {
        assert!(
            read_code(site).contains("FileDialogRequest::"),
            "{site} no longer opens a dialog through the helper; the anchor list is stale"
        );
    }
}
