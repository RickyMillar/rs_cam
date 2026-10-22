//! Geometry selection P3.3: a job `floor` key sets the floor of the op.
//!
//! The key has the project-file spelling (`floor = { from = "z", z = -2.5 }`),
//! and the CLI applies it through the `set_toolpath_floor` command after the
//! parameters. Four arms on one Pocket job (the P2.7 drawing):
//!
//! - Control: a job with no floor key cuts to the default floor, below
//!   Z -2.5.
//! - An absolute floor at Z -2.5: the lowest feed move is at Z -2.5.
//! - A Top depth of 0 refuses at the door, and the refusal names the floor.
//! - A floor on an op with no floor field (Face) refuses by name.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code; a failed fixture is a failed test."
)]

use std::path::{Path, PathBuf};

/// The absolute floor Z the second arm asks for (mm, setup frame).
const FLOOR_Z: f64 = -2.5;
/// The G-code prints Z with three decimals.
const PRINT_TOLERANCE: f64 = 1e-3;

fn drawing() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf")
        .canonicalize()
        .expect("the fixture exists")
}

/// Run one job of op `op_type` with the extra `lines`, and return the run
/// and the Z of every feed move in the G-code.
fn run_job(name: &str, op_type: &str, lines: &str) -> (std::process::Output, Vec<f64>) {
    let dir = std::env::temp_dir().join(format!("rs_cam_p3_3_{name}_{}", std::process::id()));
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
type = "{op_type}"
input = "{input}"
tool = "flat_3mm"
{lines}
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
    let zs = std::fs::read_to_string(dir.join("out.nc"))
        .map(|g| feed_zs(&g))
        .unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    (run, zs)
}

/// The Z of every G1/G2/G3 move that states a Z, with modal motion codes.
fn feed_zs(gcode: &str) -> Vec<f64> {
    let mut mode = 0_u32;
    let mut zs = Vec::new();
    for line in gcode.lines() {
        let code = line.split(';').next().unwrap_or_default();
        let mut z = None;
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
                "Z" => z = value.parse::<f64>().ok(),
                _ => {}
            }
        }
        if let Some(z) = z
            && mode != 0
        {
            zs.push(z);
        }
    }
    zs
}

fn lowest(zs: &[f64]) -> f64 {
    zs.iter().copied().fold(f64::INFINITY, f64::min)
}

#[test]
fn a_job_file_sets_a_floor_p3_3() {
    // Control: the default floor is below the floor the next arm asks for,
    // so the next arm measures the key and not the default.
    let (run, zs) = run_job("default", "pocket", "");
    assert!(
        run.status.success(),
        "the default job runs: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(!zs.is_empty(), "the default pocket cut nothing");
    assert!(
        lowest(&zs) < FLOOR_Z - PRINT_TOLERANCE,
        "the default floor {} must be below Z {FLOOR_Z} (control)",
        lowest(&zs)
    );

    // The floor key: the lowest feed move is at the absolute floor.
    let (run, zs) = run_job(
        "absolute",
        "pocket",
        &format!("floor = {{ from = \"z\", z = {FLOOR_Z} }}"),
    );
    assert!(
        run.status.success(),
        "the floor job runs: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(!zs.is_empty(), "the floor pocket cut nothing");
    assert!(
        (lowest(&zs) - FLOOR_Z).abs() <= PRINT_TOLERANCE,
        "the lowest feed move is at Z {}, not at the floor Z {FLOOR_Z}",
        lowest(&zs)
    );

    // A Top depth of 0 puts the floor at the top: the door refuses it.
    let (run, _) = run_job("zero", "pocket", "floor = { from = \"top\", depth = 0.0 }");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(!run.status.success(), "a zero depth must refuse: {stderr}");
    assert!(
        stderr.contains("floor"),
        "the refusal names the floor: {stderr}"
    );

    // An op with no floor field refuses the key by name.
    let (run, _) = run_job("face", "face", "floor = { from = \"z\", z = -1.0 }");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(!run.status.success(), "a Face floor must refuse: {stderr}");
    assert!(
        stderr.contains("has no floor field"),
        "the refusal names the missing floor: {stderr}"
    );
}
