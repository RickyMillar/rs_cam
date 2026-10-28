//! F7: a job `[operation.boundary_selection]` writes the boundary selection
//! of the op. The CLI half of the parity: the `job.rs` unit test
//! `a_job_boundary_selection_replays_one_selection_f7` holds the stored
//! selection across a save and a load and compares `selection_status`
//! with the core `EditSelection` door (the door MCP drives); this file
//! runs the real binary.
//!
//! - A Pocket job with `[operation.boundary]` (source `selection`) and a
//!   `boundary_selection` rule on layer `CUT` cuts no move past the CUT
//!   square (X 20 + the tool radius), although the op selection is its
//!   default (both squares).
//! - A `boundary_selection` on a boundary whose source is not a selection
//!   refuses with the text MCP gives.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code; a failed fixture is a failed test."
)]

use std::path::{Path, PathBuf};

/// The X where the `CUT` square ends (mm).
const CUT_MAX_X: f64 = 20.0;
/// The tool radius (mm): a centre-containment boundary keeps the tool
/// centre inside, so a feed move reaches at most this far past the edge.
const TOOL_RADIUS: f64 = 1.5;

fn drawing() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf")
        .canonicalize()
        .expect("the fixture exists")
}

const BOUNDARY: &str = r#"[operation.boundary]
enabled = true
containment = "center"
offset = 0.0
source = { selection = { selection = {} } }
"#;

const RULE: &str = r#"[[operation.boundary_selection.rules]]
source = "toolpath_model"
filter = { layers = ["CUT"] }
"#;

/// Run one Pocket job with `tail` (TOML tables) and return the run and
/// the X of every feed move in the G-code.
fn run_job(name: &str, tail: &str) -> (std::process::Output, Vec<f64>) {
    let dir = std::env::temp_dir().join(format!("rs_cam_f7_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let job = format!(
        r#"
[job]
output = "out.nc"

[tools.flat_3mm]
type = "end_mill"
diameter = 3.0

[[operation]]
type = "pocket"
input = "{input}"
tool = "flat_3mm"
stock_z = 25.0
floor = {{ from = "top", depth = 3.0 }}
{tail}
"#,
        input = drawing().display(),
    );
    let job_path = dir.join("job.toml");
    std::fs::write(&job_path, job).unwrap();
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_rs_cam_cli"))
        .arg("job")
        .arg(&job_path)
        .output()
        .expect("the CLI runs");
    let xs = std::fs::read_to_string(dir.join("out.nc"))
        .map(|g| feed_xs(&g))
        .unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    (run, xs)
}

/// The X of every G1/G2/G3 move, with modal motion codes.
fn feed_xs(gcode: &str) -> Vec<f64> {
    let mut mode = 0_u32;
    let mut xs = Vec::new();
    for line in gcode.lines() {
        let code = line.split(';').next().unwrap_or_default();
        let mut x = None;
        for word in code.split_whitespace() {
            let (letter, value) = word.split_at(1);
            match letter {
                "G" => {
                    if let Ok(g) = value.parse::<u32>()
                        && g <= 3
                    {
                        mode = g;
                    }
                }
                "X" => x = value.parse::<f64>().ok(),
                _ => {}
            }
        }
        if let Some(x) = x
            && mode != 0
        {
            xs.push(x);
        }
    }
    xs
}

#[test]
fn a_job_boundary_selection_confines_the_cut_f7() {
    let (run, xs) = run_job("rule", &format!("{BOUNDARY}{RULE}"));
    assert!(
        run.status.success(),
        "the job runs: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(!xs.is_empty(), "the pocket cut nothing");
    let right = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        right <= CUT_MAX_X + TOOL_RADIUS + 1e-6,
        "a feed move reached X {right}: the boundary selection did not confine the cut"
    );
}

#[test]
fn a_boundary_selection_on_a_stock_boundary_refuses_f7() {
    let (run, _) = run_job("stock", RULE);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(!run.status.success(), "the job must refuse: {stderr}");
    // The core (and MCP) text of `boundary_not_a_selection`.
    assert!(
        stderr.contains("boundary_selection:")
            && stderr.contains("is not a selection: set the boundary source to Selection"),
        "the refusal gives the MCP text: {stderr}"
    );
}
