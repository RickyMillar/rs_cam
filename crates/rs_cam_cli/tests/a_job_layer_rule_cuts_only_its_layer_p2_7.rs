//! Geometry selection P2.7: a job `[operation.selection]` cuts only what it
//! selects. The CLI half of the core sentry
//! `a_layer_rule_cuts_only_its_layer_p2_3`, on a real DXF.
//!
//! The drawing (`rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf`)
//! holds a 20 mm square on layer `CUT` (X 0 to 20), a 20 mm square on layer
//! `KEEP` (X 40 to 60), two circles on layer `HOLES` (X 80 and 100) and an
//! open line on layer `TEXT`.
//!
//! - A Pocket job with no selection cuts the `KEEP` square too (control).
//! - A Pocket job with one rule on layer `CUT` cuts no move past X 20.
//! - A rule on a layer the drawing does not hold refuses and names it.
//!
//! The job key takes the shape of the project file's
//! `operation.params.selection`, and the CLI applies it through the
//! `edit_toolpath_selection` command.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code; a failed fixture is a failed test."
)]

use std::path::{Path, PathBuf};

/// The X where the `CUT` square ends (mm).
const CUT_MAX_X: f64 = 20.0;
/// The X where the `KEEP` square starts (mm).
const KEEP_MIN_X: f64 = 40.0;

fn drawing() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf")
        .canonicalize()
        .expect("the fixture exists")
}

/// Run one Pocket job with `selection` (TOML lines, or empty) and return
/// the run and the X of every feed move in the G-code.
fn run_job(name: &str, selection: &str) -> (std::process::Output, Vec<f64>) {
    let dir = std::env::temp_dir().join(format!("rs_cam_p2_7_{name}_{}", std::process::id()));
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
{selection}
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
fn a_job_with_no_selection_cuts_every_square() {
    let (run, xs) = run_job("default", "");
    assert!(
        run.status.success(),
        "the job runs: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        xs.iter().any(|x| *x > KEEP_MIN_X),
        "the default selection must cut the KEEP square (control): {xs:?}"
    );
}

#[test]
fn a_job_layer_rule_cuts_only_its_layer() {
    let (run, xs) = run_job(
        "rule",
        r#"[[operation.selection.rules]]
source = "toolpath_model"
filter = { layers = ["CUT"] }"#,
    );
    assert!(
        run.status.success(),
        "the job runs: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(!xs.is_empty(), "the pocket cut nothing");
    let right = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        right <= CUT_MAX_X,
        "a feed move reached X {right}: an item off layer CUT was cut"
    );
}

#[test]
fn a_job_rule_on_an_absent_layer_refuses_and_names_it() {
    let (run, _) = run_job(
        "absent",
        r#"[[operation.selection.rules]]
source = "toolpath_model"
filter = { layers = ["NO_SUCH_LAYER"] }"#,
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(!run.status.success(), "the job must refuse: {stderr}");
    assert!(
        stderr.contains("NO_SUCH_LAYER"),
        "the refusal names the layer: {stderr}"
    );
}
