//! Geometry selection P6.8 (design section 8, the CLI job file row): a job
//! `[[operation]]` of `type = "bore"` bores the circles larger than the
//! tool, and the job key `open_paths` is read as a Pocket parameter.
//!
//! The drawing (`rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf`)
//! holds two squares (Y 0 to 20), two 6 mm circles on layer `HOLES` at
//! (80, 10) and (100, 10), and an open line on layer `TEXT` drawn east
//! along Y 40, from X 0 to X 60.
//!
//! - Bore with a 4 mm end mill: every feed move is within one helix radius
//!   `r = (d - D) / 2 = 1 mm` of a circle centre, and each circle has a
//!   full helix at that radius (design 4.2).
//! - Bore with a 6 mm end mill: each circle is the tool size, so the Bore
//!   leaves both out and refuses with nothing to cut.
//! - Pocket `open_paths = "skip"` (control): no feed move above Y 20.
//! - `"extend_left"`: the area north of the line (left of the drawn
//!   direction) is cut, and no move lies between the squares and the line.
//! - `"extend_right"`: the area south of the line is cut (Y 21 to 39).
//! - An unknown token refuses, and the refusal names the three tokens.
//!
//! ```text
//! cargo test -p rs_cam_cli -q --test a_job_file_bores_a_hole_p6_8
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code; a failed fixture is a failed test."
)]

use std::path::{Path, PathBuf};

/// The circle centres of layer `HOLES` (mm).
const CENTRES: [[f64; 2]; 2] = [[80.0, 10.0], [100.0, 10.0]];
/// The helix radius of a 6 mm circle and a 4 mm tool: (6 - 4) / 2 (mm).
const HELIX_R: f64 = 1.0;
/// The G-code prints X and Y with three decimals; a helix point is
/// rounded on each axis.
const PRINT_TOLERANCE: f64 = 2e-3;
/// The Y of the open line, and the top Y of the squares (mm).
const LINE_Y: f64 = 40.0;
const SQUARES_MAX_Y: f64 = 20.0;

fn drawing() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf")
        .canonicalize()
        .expect("the fixture exists")
}

/// Run one job of `op_type` with an end mill of `tool_d` and the extra
/// `lines`; return the run and the XY of every feed move in the G-code.
fn run_job(
    name: &str,
    op_type: &str,
    tool_d: f64,
    lines: &str,
) -> (std::process::Output, Vec<[f64; 2]>) {
    let dir = std::env::temp_dir().join(format!("rs_cam_p6_8_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let job = format!(
        r#"
[job]
output = "out.nc"

[tools.flat]
type = "end_mill"
diameter = {tool_d}

[[operation]]
type = "{op_type}"
input = "{input}"
tool = "flat"
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
    let xys = std::fs::read_to_string(dir.join("out.nc"))
        .map(|g| feed_xys(&g))
        .unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    (run, xys)
}

/// The XY of every G1/G2/G3 line that states X or Y, with modal motion
/// codes and modal coordinates.
fn feed_xys(gcode: &str) -> Vec<[f64; 2]> {
    let (mut mode, mut x, mut y) = (0_u32, None, None);
    let mut out = Vec::new();
    for line in gcode.lines() {
        let code = line.split(';').next().unwrap_or_default();
        let mut moved = false;
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
                "X" => {
                    x = value.parse::<f64>().ok();
                    moved = true;
                }
                "Y" => {
                    y = value.parse::<f64>().ok();
                    moved = true;
                }
                _ => {}
            }
        }
        if moved
            && mode != 0
            && let (Some(x), Some(y)) = (x, y)
        {
            out.push([x, y]);
        }
    }
    out
}

fn distance(p: [f64; 2], c: [f64; 2]) -> f64 {
    (p[0] - c[0]).hypot(p[1] - c[1])
}

fn assert_ran(run: &std::process::Output, what: &str) {
    assert!(
        run.status.success(),
        "{what} runs: {}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn a_job_file_bores_a_hole_p6_8() {
    // A 4 mm tool bores each 6 mm circle with a helix of radius 1 mm.
    let (run, xys) = run_job("bore", "bore", 4.0, "");
    assert_ran(&run, "the bore job");
    assert!(!xys.is_empty(), "the bore cut nothing");
    for p in &xys {
        let nearest = CENTRES
            .iter()
            .map(|&c| distance(*p, c))
            .fold(f64::INFINITY, f64::min);
        assert!(
            nearest <= HELIX_R + PRINT_TOLERANCE,
            "a feed move at {p:?} is {nearest:.4} mm from a centre, past the helix \
             radius {HELIX_R}"
        );
    }
    for c in CENTRES {
        let on_helix = xys
            .iter()
            .filter(|&&p| (distance(p, c) - HELIX_R).abs() <= PRINT_TOLERANCE)
            .count();
        assert!(
            on_helix >= 36,
            "the circle at {c:?} has {on_helix} feed points on the helix, not one turn"
        );
    }

    // A 6 mm tool: each circle is the tool size, so the Bore leaves it out
    // and has nothing to cut.
    let (run, _) = run_job("bore_tool_size", "bore", 6.0, "");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        !run.status.success(),
        "a Bore with only tool-size circles must refuse: {stderr}"
    );
    assert!(
        stderr.contains("Bore"),
        "the refusal names the op: {stderr}"
    );

    // The job key `open_paths` is a Pocket parameter.
    let (run, xys) = run_job("skip", "pocket", 3.0, "open_paths = \"skip\"");
    assert_ran(&run, "the skip job");
    assert!(!xys.is_empty(), "the pocket cut nothing");
    assert!(
        xys.iter().all(|p| p[1] <= SQUARES_MAX_Y),
        "Skip cuts the squares only (control)"
    );

    let (run, xys) = run_job("left", "pocket", 3.0, "open_paths = \"extend_left\"");
    assert_ran(&run, "the extend_left job");
    assert!(
        xys.iter().any(|p| p[1] > LINE_Y),
        "extend_left cuts north of the line"
    );
    assert!(
        !xys.iter()
            .any(|p| p[1] > SQUARES_MAX_Y + 1.0 && p[1] < LINE_Y - 1.0),
        "extend_left cuts nothing south of the line"
    );

    let (run, xys) = run_job("right", "pocket", 3.0, "open_paths = \"extend_right\"");
    assert_ran(&run, "the extend_right job");
    assert!(
        xys.iter()
            .any(|p| p[1] > SQUARES_MAX_Y + 1.0 && p[1] < LINE_Y - 1.0),
        "extend_right cuts south of the line"
    );

    let (run, _) = run_job("bogus", "pocket", 3.0, "open_paths = \"bogus\"");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(!run.status.success(), "an unknown token refuses: {stderr}");
    for token in ["open_paths", "skip", "extend_left", "extend_right"] {
        assert!(
            stderr.contains(token),
            "the refusal names `{token}`: {stderr}"
        );
    }
}
