//! W6 (toolpath inspector programme, 2026-10-30): the issue stack shows at
//! most three rows plus the blockers, and the notes counter is the only
//! place a note paints.
//!
//! # The claims
//!
//! 1. `NoticeStack::issues` (THE issue stack of the inspector) paints at
//!    most `ISSUE_CAP` (3) rows, or more only for Danger rows: every Danger
//!    row paints, and a Caution row never takes a row over the cap. The
//!    overflow line states the true total. Measured on the real component
//!    with 6 Caution and 2 Danger rows, and with 5 Danger and 4 Caution
//!    rows (no W0 state has more than three issues, so a panel pass alone
//!    would be vacuous).
//! 2. In every W0 bad state, on every tab, no face of a note finding
//!    (`placement::place` gives Note, Info and Hint findings to the notes
//!    counter) paints: the counter shows "ⓘ n" and the faces are in its
//!    popover. The issue faces that paint are at most the cap plus the
//!    Danger findings.
//! 3. Source: in the inspector only `toolpath_panel.rs` reads the notes
//!    (`Placement::note_notices`, once), and only the kit `StatusLine`
//!    draws `NotesCounter`.
//!
//! Red proof: paint a note face as an issue row (`placement.notes` into
//! `draw_issue_stack`): claim 2 fails. Raise `ISSUE_CAP`: claim 1 fails.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_issue_stack_is_capped_w6
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod inspector_bad_states;

use inspector_bad_states::{Family, State, fixture, screen};
use rs_cam_viz::ui::components::{Notice, NoticeStack, Role};
use rs_cam_viz::ui::properties::{self, ToolpathTab};

/// The faces the issue stack paints for `notices`, collapsed.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn painted_faces(notices: &[Notice]) -> Vec<String> {
    let ctx = egui::Context::default();
    rs_cam_viz::ui::tokens::apply(&ctx);
    rs_cam_viz::ui::tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.set_max_width(340.0);
        let _ = NoticeStack::issues(notices.to_vec()).show_issues(ui);
    });
    let faces = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.job.text.clone()),
            _ => None,
        })
        .collect();
    out.textures_delta.clear();
    faces
}

fn stack(dangers: usize, cautions: usize) -> Vec<Notice> {
    (0..cautions)
        .map(|i| Notice::new(Role::Caution, format!("Caution finding {i}")))
        .chain((0..dangers).map(|i| Notice::new(Role::Danger, format!("Danger finding {i}"))))
        .collect()
}

/// Claim 1.
#[test]
fn the_issue_stack_shows_three_rows_plus_blockers_w6() {
    assert_eq!(NoticeStack::ISSUE_CAP, 3, "W1: three issue rows");
    for (dangers, cautions) in [(2, 6), (5, 4), (0, 7)] {
        let faces = painted_faces(&stack(dangers, cautions));
        let danger_rows = faces.iter().filter(|f| f.starts_with("Danger")).count();
        let caution_rows = faces.iter().filter(|f| f.starts_with("Caution")).count();
        assert_eq!(
            danger_rows, dangers,
            "{dangers} Danger + {cautions} Caution: every Danger row paints: {faces:?}"
        );
        assert_eq!(
            danger_rows + caution_rows,
            NoticeStack::ISSUE_CAP.max(dangers),
            "{dangers} Danger + {cautions} Caution: the cap is 3, or the Danger count: {faces:?}"
        );
        let total = dangers + cautions;
        assert!(
            faces.iter().any(|f| f.contains(&total.to_string())),
            "{dangers} Danger + {cautions} Caution: the overflow line states the total \
             {total}: {faces:?}"
        );
    }
}

/// Claim 2.
#[test]
fn a_note_paints_only_in_the_notes_counter_w6() {
    let mut failures = Vec::new();
    let mut states_with_notes = 0_usize;
    let mut counters = 0_usize;
    let mut screens = 0_usize;
    for family in Family::ALL {
        for bad in State::ALL {
            if bad.skip_reason(family).is_some() {
                continue;
            }
            let (mut state, id) = fixture(family, bad);
            // The first render fills the feeds memo, whose findings (the
            // feeds notes) join the list; place after it.
            let _ = screen(&mut state, id, ToolpathTab::FeedsSpeeds);
            let placed = properties::placed_findings(id, &state).expect("the toolpath exists");
            states_with_notes += usize::from(!placed.notes.is_empty());
            let note_faces: Vec<&str> = placed.notes.iter().map(|d| d.face.as_str()).collect();
            let issue_faces: Vec<&str> = placed.issues.iter().map(|d| d.face.as_str()).collect();
            let dangers = placed
                .issues
                .iter()
                .filter(|d| {
                    matches!(
                        d.severity,
                        rs_cam_core::diagnostics::Severity::Blocking
                            | rs_cam_core::diagnostics::Severity::Critical
                    )
                })
                .count();
            for tab in ToolpathTab::ALL {
                screens += 1;
                let painted = screen(&mut state, id, *tab);
                let counter = rs_cam_viz::ui::components::NotesCounter::text(placed.notes.len());
                counters += usize::from(
                    !placed.notes.is_empty() && painted.iter().any(|p| p.text == counter),
                );
                for p in &painted {
                    if note_faces.contains(&p.text.as_str()) {
                        failures.push(format!(
                            "{} / {} / {tab:?}: the note {:?} paints; it belongs in the \
                             notes counter",
                            family.name(),
                            bad.name(),
                            p.text
                        ));
                    }
                }
                let issues = painted
                    .iter()
                    .filter(|p| issue_faces.contains(&p.text.as_str()))
                    .count();
                if issues > NoticeStack::ISSUE_CAP.max(dangers) {
                    failures.push(format!(
                        "{} / {} / {tab:?}: {issues} issue rows, {dangers} blockers",
                        family.name(),
                        bad.name()
                    ));
                }
            }
        }
    }
    assert!(screens > 100, "non-vacuity: {screens} screens");
    assert!(
        states_with_notes > 0 && counters > 0,
        "non-vacuity: {states_with_notes} states with notes, {counters} screens paint the \
         counter"
    );
    failures.sort();
    failures.dedup();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Claim 3.
#[test]
fn only_the_status_line_draws_the_notes_counter_w6() {
    let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let read = |rel: &str| {
        std::fs::read_to_string(ui.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
    };
    let code = |src: &str| -> String {
        src.lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let status_line = code(&read("components/status_line.rs"));
    assert!(
        status_line.contains("NotesCounter::new(notes)"),
        "non-vacuity: the status line draws the counter"
    );
    let mut stack = vec![ui.join("properties")];
    let mut readers = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read a folder") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            // `placement.rs` builds the notes; `tests.rs` is test code;
            // `mod.rs` copies them for the `placed_findings` test door.
            if path.extension().is_none_or(|e| e != "rs")
                || path.ends_with("tests.rs")
                || path.ends_with("placement.rs")
                || path.ends_with("properties/mod.rs")
            {
                continue;
            }
            let src = code(&std::fs::read_to_string(&path).expect("read a file"));
            assert!(
                !src.contains("NotesCounter"),
                "{} draws the notes counter; the kit StatusLine owns it",
                path.display()
            );
            let n = src.matches(".note_notices()").count() + src.matches("placement.notes").count();
            if n > 0 {
                readers.push((path.display().to_string(), n));
            }
        }
    }
    assert_eq!(
        readers.len(),
        1,
        "one inspector file reads the notes: {readers:?}"
    );
    let (file, n) = readers.first().expect("one reader");
    assert!(
        file.ends_with("properties/toolpath_panel.rs") && *n == 1,
        "the tab host reads the notes once: {readers:?}"
    );
}
