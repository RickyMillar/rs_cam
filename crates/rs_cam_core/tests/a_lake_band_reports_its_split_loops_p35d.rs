//! P35 D, the evidence run: the split report of one Profile of a project
//! file. It generates the toolpath `P35D_TOOLPATH` (a name) of the project
//! `P35D_PROJECT` (a path) and prints the notes of the op: each loop that
//! the radial stock to leave splits, with its start point and its ring
//! count. A loop with no note is cut as one ring.
//!
//! ```text
//! P35D_PROJECT=<file.toml> P35D_TOOLPATH="<name>" cargo test -p rs_cam_core \
//!     --test a_lake_band_reports_its_split_loops_p35d -- --ignored --nocapture
//! ```

#![expect(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "an evidence run: a panic reports a failure, and the printed notes are the result"
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::session::ProjectSession;

#[test]
#[ignore = "evidence: reads the project file that P35D_PROJECT names (an operator job)"]
fn the_split_loops_of_one_profile_p35d() {
    let project = std::env::var("P35D_PROJECT").expect("P35D_PROJECT names the project file");
    let name = std::env::var("P35D_TOOLPATH").expect("P35D_TOOLPATH names the toolpath");
    let mut session = ProjectSession::load(std::path::Path::new(&project)).expect("load");
    let index = session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.name == name)
        .expect("the project holds the toolpath");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(index, &cancel)
        .expect("the toolpath generates");
    let result = session.result_at(index).expect("a result");
    eprintln!("{name}: {} moves", result.annotated().toolpath.moves.len());
    match &result.stats.tab_placement {
        None => eprintln!("no note: each loop is one ring"),
        Some(finding) => {
            for note in &finding.notes {
                eprintln!("note: {note}");
            }
        }
    }
    let diagnostics = session
        .diagnose_toolpath_with_trace(index, None)
        .expect("diagnose");
    for d in &diagnostics {
        eprintln!(
            "diagnostic {} ({:?}): {}",
            d.id.as_str(),
            d.severity,
            d.face
        );
    }
}
