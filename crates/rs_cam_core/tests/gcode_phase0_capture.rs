//! G-code byte goldens (G9, 2026-10-08).
//!
//! Each fixture in `common::gcode_fixtures` is emitted for each shipped
//! post and compared byte for byte with its golden in
//! `tests/fixtures/gcode_golden/<fixture>_<dialect>.nc`. A change to the
//! emitted output fails this test. The test is not ignored.
//!
//! Before G9 this file only WROTE the captures (under `planning/`), and
//! no test compared them, so an emitter change went unseen.
//!
//! To re-bless after a change with a measured cause, run:
//!
//! ```bash
//! RS_CAM_BLESS_GCODE=1 cargo test -p rs_cam_core --test gcode_phase0_capture
//! ```
//!
//! Then review the diff of `tests/fixtures/gcode_golden/` in the same
//! commit. A fixture that a post refuses has no golden file; the test
//! asserts that the refusal is still there.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::gcode_fixtures::{DIALECTS, FIXTURES, golden_path};
use rs_cam_core::gcode::WizardOverlay;

fn blessing() -> bool {
    std::env::var("RS_CAM_BLESS_GCODE").is_ok_and(|v| v == "1")
}

#[test]
fn emitted_gcode_matches_the_goldens() {
    let bless = blessing();
    let mut failures: Vec<String> = Vec::new();
    let mut compared = 0usize;
    let mut refused = 0usize;

    for (fixture, build) in FIXTURES {
        for (dialect, format) in DIALECTS {
            let path = golden_path(fixture, dialect);
            let emitted = build(format.definition(), &WizardOverlay::default());
            match emitted {
                Ok(gcode) => {
                    compared += 1;
                    if bless {
                        std::fs::create_dir_all(path.parent().expect("golden dir"))
                            .expect("create golden dir");
                        std::fs::write(&path, &gcode).expect("write golden");
                        continue;
                    }
                    let Ok(golden) = std::fs::read_to_string(&path) else {
                        failures.push(format!("{}: golden missing", path.display()));
                        continue;
                    };
                    if golden != gcode {
                        let first = golden
                            .lines()
                            .zip(gcode.lines())
                            .position(|(a, b)| a != b)
                            .map_or_else(
                                || "line count differs".to_owned(),
                                |i| {
                                    format!(
                                        "line {}: golden {:?} / emitted {:?}",
                                        i + 1,
                                        golden.lines().nth(i).unwrap_or(""),
                                        gcode.lines().nth(i).unwrap_or("")
                                    )
                                },
                            );
                        failures.push(format!("{fixture}_{dialect}: {first}"));
                    }
                }
                Err(_) => {
                    refused += 1;
                    if bless && path.exists() {
                        std::fs::remove_file(&path).expect("remove stale golden");
                    }
                    if !bless && path.exists() {
                        failures.push(format!(
                            "{fixture}_{dialect}: the door refused, but a golden exists"
                        ));
                    }
                }
            }
        }
    }

    // Non-vacuity: the corpus must emit something on every dialect.
    assert!(compared >= 60, "only {compared} programs compared");
    assert!(
        failures.is_empty(),
        "{} golden mismatch(es) ({compared} compared, {refused} refused). \
         Re-bless with RS_CAM_BLESS_GCODE=1 only for a measured cause:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}
