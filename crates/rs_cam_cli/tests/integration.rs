//! Integration tests for the rs_cam CLI crate.
//!
//! These tests exercise job parsing, tool construction, operation execution,
//! and G-code output without spawning subprocesses.

// Re-use the public API from the job module (re-exported by the crate's lib-like structure).
// Since rs_cam_cli is a binary crate, we test by importing rs_cam_core directly
// and duplicating the minimal parsing logic that the CLI uses.

use rs_cam_core::gcode::{emit_gcode, get_post_processor};
use rs_cam_core::pocket::{PocketParams, pocket_toolpath};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::{
    BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, VBitEndmill,
};

// ---------------------------------------------------------------------------
// Helper: parse a tool spec string the same way the CLI does
// ---------------------------------------------------------------------------

fn parse_tool_spec(spec: &str) -> Result<Box<dyn MillingCutter>, String> {
    let parts: Vec<&str> = spec.split(':').collect();
    if parts.len() < 2 {
        return Err(
            "Tool spec must be type:diameter[:params] (e.g., ball:6.35)".to_string(),
        );
    }

    let diameter: f64 = parts[1]
        .parse()
        .map_err(|_| "Invalid tool diameter".to_string())?;
    let cutting_length = diameter * 4.0;

    match parts[0] {
        "ball" => Ok(Box::new(BallEndmill::new(diameter, cutting_length))),
        "flat" => Ok(Box::new(FlatEndmill::new(diameter, cutting_length))),
        "bullnose" => {
            let corner_radius: f64 = parts
                .get(2)
                .ok_or("Bull nose needs corner radius: bullnose:10:2")?
                .parse()
                .map_err(|_| "Invalid corner radius".to_string())?;
            Ok(Box::new(BullNoseEndmill::new(
                diameter,
                corner_radius,
                cutting_length,
            )))
        }
        "vbit" => {
            let angle: f64 = parts
                .get(2)
                .ok_or("V-bit needs included angle: vbit:10:90")?
                .parse()
                .map_err(|_| "Invalid included angle".to_string())?;
            Ok(Box::new(VBitEndmill::new(diameter, angle, cutting_length)))
        }
        "tapered_ball" => {
            let taper_angle: f64 = parts
                .get(2)
                .ok_or("Tapered ball needs taper angle: tapered_ball:6:10:12")?
                .parse()
                .map_err(|_| "Invalid taper half-angle".to_string())?;
            let shaft_diameter: f64 = parts
                .get(3)
                .ok_or("Tapered ball needs shaft diameter: tapered_ball:6:10:12")?
                .parse()
                .map_err(|_| "Invalid shaft diameter".to_string())?;
            Ok(Box::new(TaperedBallEndmill::new(
                diameter,
                taper_angle,
                shaft_diameter,
                cutting_length,
            )))
        }
        _ => Err(format!("Unknown tool type '{}'", parts[0])),
    }
}

// ---------------------------------------------------------------------------
// Test 1: Parse tool spec — flat endmill
// ---------------------------------------------------------------------------

#[test]
fn test_parse_tool_spec_flat() {
    let cutter = parse_tool_spec("flat:6.35").expect("Should parse flat:6.35");
    assert!(
        (cutter.diameter() - 6.35).abs() < 1e-10,
        "Flat endmill diameter should be 6.35, got {}",
        cutter.diameter()
    );
    assert!(
        (cutter.radius() - 3.175).abs() < 1e-10,
        "Flat endmill radius should be 3.175, got {}",
        cutter.radius()
    );
}

// ---------------------------------------------------------------------------
// Test 2: Parse tool spec — ball endmill
// ---------------------------------------------------------------------------

#[test]
fn test_parse_tool_spec_ball() {
    let cutter = parse_tool_spec("ball:10.0").expect("Should parse ball:10.0");
    assert!(
        (cutter.diameter() - 10.0).abs() < 1e-10,
        "Ball endmill diameter should be 10.0"
    );
    assert!(
        (cutter.radius() - 5.0).abs() < 1e-10,
        "Ball endmill radius should be 5.0"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Parse tool spec — bullnose with corner radius
// ---------------------------------------------------------------------------

#[test]
fn test_parse_tool_spec_bullnose() {
    let cutter = parse_tool_spec("bullnose:10:2").expect("Should parse bullnose:10:2");
    assert!(
        (cutter.diameter() - 10.0).abs() < 1e-10,
        "Bull nose diameter should be 10.0"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Parse tool spec — v-bit with included angle
// ---------------------------------------------------------------------------

#[test]
fn test_parse_tool_spec_vbit() {
    let cutter = parse_tool_spec("vbit:10:90").expect("Should parse vbit:10:90");
    assert!(
        (cutter.diameter() - 10.0).abs() < 1e-10,
        "V-bit diameter should be 10.0"
    );
}

// ---------------------------------------------------------------------------
// Test 5: Parse tool spec — tapered ball
// ---------------------------------------------------------------------------

#[test]
fn test_parse_tool_spec_tapered_ball() {
    let cutter = parse_tool_spec("tapered_ball:6:10:12").expect("Should parse tapered_ball:6:10:12");
    // TaperedBallEndmill::diameter() returns shaft_diameter
    assert!(
        (cutter.diameter() - 12.0).abs() < 1e-10,
        "Tapered ball diameter() should return shaft diameter 12.0, got {}",
        cutter.diameter()
    );
}

// ---------------------------------------------------------------------------
// Test 6: Parse tool spec — invalid specs produce errors
// ---------------------------------------------------------------------------

#[test]
fn test_parse_tool_spec_invalid() {
    assert!(
        parse_tool_spec("flat").is_err(),
        "Missing diameter should fail"
    );
    assert!(
        parse_tool_spec("unknown:6.35").is_err(),
        "Unknown tool type should fail"
    );
    assert!(
        parse_tool_spec("bullnose:10").is_err(),
        "Bullnose without corner_radius should fail"
    );
    assert!(
        parse_tool_spec("vbit:10").is_err(),
        "V-bit without angle should fail"
    );
    assert!(
        parse_tool_spec("tapered_ball:6:10").is_err(),
        "Tapered ball without shaft_diameter should fail"
    );
    assert!(
        parse_tool_spec("flat:abc").is_err(),
        "Non-numeric diameter should fail"
    );
}

// ---------------------------------------------------------------------------
// Test 7: Run a pocket operation and verify G-code output
// ---------------------------------------------------------------------------

#[test]
fn test_pocket_operation_produces_gcode() {
    // Simple rectangular polygon
    let polygon = Polygon2::rectangle(0.0, 0.0, 40.0, 30.0);
    let params = PocketParams {
        tool_radius: 3.175,
        stepover: 2.0,
        cut_depth: -3.0,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 10.0,
        climb: false,
    };

    let tp = pocket_toolpath(&polygon, &params);
    assert!(
        !tp.moves.is_empty(),
        "Pocket toolpath should have moves for a 40x30mm rectangle"
    );

    let post = get_post_processor("grbl").expect("GRBL post should exist");
    let gcode = emit_gcode(&tp, post.as_ref(), 18000);

    // G-code should be non-empty and contain expected patterns
    assert!(!gcode.is_empty(), "G-code should not be empty");
    assert!(gcode.contains("G0"), "G-code should contain rapid moves (G0)");
    assert!(
        gcode.contains("G1"),
        "G-code should contain linear feed moves (G1)"
    );
    assert!(
        gcode.contains("M3 S18000"),
        "G-code should contain spindle start"
    );
    assert!(gcode.contains("M30"), "G-code should contain program end");
    // The cut depth should appear in the G-code as Z-3.000
    assert!(
        gcode.contains("Z-3.000"),
        "G-code should contain the cut depth Z-3.000"
    );
}

// ---------------------------------------------------------------------------
// Test 8: Parse demo job TOML inline and validate structure
// ---------------------------------------------------------------------------

#[test]
fn test_parse_demo_job_toml() {
    // Minimal job TOML that mirrors the structure of fixtures/demo_job.toml
    let toml_content = r#"
[job]
output = "test_output.nc"
post = "grbl"
spindle_speed = 18000
safe_z = 10.0

[tools.flat_6mm]
type = "flat"
diameter = 6.35

[tools.flat_3mm]
type = "flat"
diameter = 3.175

[[operation]]
type = "pocket"
input = "demo_pocket.svg"
tool = "flat_6mm"
stepover = 2.0
depth = 6.0
depth_per_pass = 3.0
feed_rate = 2000
plunge_rate = 500

[[operation]]
type = "profile"
input = "demo_pocket.svg"
tool = "flat_3mm"
depth = 6.0
depth_per_pass = 2.0
side = "inside"
feed_rate = 1000
plunge_rate = 400
"#;

    // Parse the TOML using the same serde types the CLI uses
    let job: serde::de::IgnoredAny = toml::from_str(toml_content).unwrap();
    // We can't directly import JobFile from the binary crate, so we validate
    // by deserializing into the expected shape manually.
    let value: toml::Value = toml::from_str(toml_content).expect("Valid TOML");

    // Verify structure
    let job_section = value.get("job").expect("Should have [job] section");
    assert_eq!(
        job_section.get("post").and_then(|v| v.as_str()),
        Some("grbl")
    );
    assert_eq!(
        job_section.get("spindle_speed").and_then(|v| v.as_integer()),
        Some(18000)
    );

    let tools = value.get("tools").expect("Should have [tools] section");
    assert!(
        tools.get("flat_6mm").is_some(),
        "Should have flat_6mm tool"
    );
    assert!(
        tools.get("flat_3mm").is_some(),
        "Should have flat_3mm tool"
    );

    let ops = value
        .get("operation")
        .expect("Should have [[operation]] entries")
        .as_array()
        .expect("operations should be an array");
    assert_eq!(ops.len(), 2, "Should have 2 operations");

    // First operation references flat_6mm
    assert_eq!(
        ops[0].get("tool").and_then(|v| v.as_str()),
        Some("flat_6mm")
    );
    assert_eq!(
        ops[0].get("type").and_then(|v| v.as_str()),
        Some("pocket")
    );

    // Second operation references flat_3mm
    assert_eq!(
        ops[1].get("tool").and_then(|v| v.as_str()),
        Some("flat_3mm")
    );
    assert_eq!(
        ops[1].get("type").and_then(|v| v.as_str()),
        Some("profile")
    );

    // Verify tool references are valid (all operation tools exist in [tools])
    let tools_table = tools.as_table().expect("tools should be a table");
    for (i, op) in ops.iter().enumerate() {
        let tool_name = op
            .get("tool")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("Operation {} should have a tool", i));
        assert!(
            tools_table.contains_key(tool_name),
            "Operation {} references '{}' which should exist in [tools]",
            i,
            tool_name
        );
    }

    // Suppress unused-variable warning from the IgnoredAny parse above
    let _ = job;
}

// ---------------------------------------------------------------------------
// Test 9: TOML with missing tool reference fails validation
// ---------------------------------------------------------------------------

#[test]
fn test_job_toml_missing_tool_detected() {
    let toml_content = r#"
[job]
output = "test.nc"

[tools.flat_6mm]
type = "flat"
diameter = 6.35

[[operation]]
type = "pocket"
input = "test.svg"
tool = "nonexistent_tool"
depth = 3.0
"#;

    let value: toml::Value = toml::from_str(toml_content).expect("Valid TOML syntax");

    let tools = value
        .get("tools")
        .and_then(|t| t.as_table())
        .expect("tools section");
    let ops = value
        .get("operation")
        .and_then(|o| o.as_array())
        .expect("operations");

    // Simulate the validation that parse_job_file does
    for op in ops.iter() {
        let tool_name = op.get("tool").and_then(|v| v.as_str()).unwrap();
        if !tools.contains_key(tool_name) {
            // This is expected: nonexistent_tool is not in [tools]
            return; // Test passes
        }
    }
    panic!("Should have detected missing tool reference 'nonexistent_tool'");
}
