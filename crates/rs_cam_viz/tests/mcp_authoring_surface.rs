//! Sentries for the MCP surface an agent needs to author a job from
//! scratch — the gaps recorded in
//! `planning/airrun_2026-08-19/RUN_LOG.md` §"MCP surface gaps".
//!
//! Those gaps forced a save -> hand-edit TOML -> reload cycle twice
//! during a real job:
//!
//! - `set_toolpath_param`'s `value` had no declared type, so an array
//!   argument arrived server-side as a string and the call FAILED:
//!   `invalid type: string "[[2.5,2.5],[237.5,247.5]]", expected a
//!   sequence`. The pin-drill hole list had to be hand-written.
//! - `set_stock_config` set only x/y/z — no origin, no material, no
//!   workholding rigidity — and did not clear `auto_from_model`, so a
//!   stock set explicitly to 240x250x25 kept an origin silently
//!   re-derived from the last imported model's bounding box.
//! - `add_tool` took only name/type/diameter and filled type-AGNOSTIC
//!   defaults, so a 20-degree V-bit was created as a 90-degree one.
//! - `set_tool_param` accepted `included_angle` / `taper_half_angle`
//!   but its description did not say so.
//!
//! What these tests assert is the schema and description text the LLM
//! actually receives — read straight off the `ToolRouter` the embedded
//! server registers, with no GUI and no project. An inaccurate tool
//! description is a real defect on this surface, not a doc nit, so the
//! description assertions are load-bearing.

#![cfg(feature = "mcp")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_viz::mcp_server::EmbeddedCamServer;

/// One registered tool, reduced to the two things an agent reads.
struct ToolFacts {
    description: String,
    schema: serde_json::Value,
}

fn tool_facts(name: &str) -> ToolFacts {
    let router = EmbeddedCamServer::into_tool_router();
    let tool = router
        .list_all()
        .into_iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("the embedded server no longer registers `{name}`"));
    ToolFacts {
        description: tool.description.map(|d| d.into_owned()).unwrap_or_default(),
        schema: serde_json::Value::Object((*tool.input_schema).clone()),
    }
}

fn property<'a>(facts: &'a ToolFacts, field: &str) -> &'a serde_json::Value {
    facts
        .schema
        .pointer(&format!("/properties/{field}"))
        .unwrap_or_else(|| panic!("missing property `{field}` in {}", facts.schema))
}

fn required(facts: &ToolFacts) -> Vec<String> {
    facts
        .schema
        .pointer("/required")
        .and_then(|r| r.as_array())
        .map(|r| {
            r.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Gap 6 — the hard blocker: an untyped `value` meant array arguments
/// never reached the server as arrays. The schema must now name the
/// types it accepts, and `array` must be one of them.
#[test]
fn param_setters_declare_a_typed_value_that_admits_arrays() {
    for tool in ["set_toolpath_param", "set_tool_param", "set_dressup_field"] {
        let facts = tool_facts(tool);
        let value = property(&facts, "value");
        assert!(
            value.is_object(),
            "{tool}: `value` must carry a real schema, not the untyped `true`: {value}"
        );
        let types: Vec<&str> = value
            .pointer("/type")
            .and_then(|t| t.as_array())
            .unwrap_or_else(|| panic!("{tool}: `value` must declare a type list: {value}"))
            .iter()
            .filter_map(|t| t.as_str())
            .collect();
        for expected in ["array", "object", "number", "string", "boolean"] {
            assert!(
                types.contains(&expected),
                "{tool}: `value` must accept {expected} — {types:?}"
            );
        }
    }
}

/// Gap 5 — doc-only, but this surface is READ BY AN LLM, so a
/// description that omits an accepted parameter is a defect: the agent
/// concludes the parameter does not exist.
#[test]
fn set_tool_param_description_lists_every_parameter_it_accepts() {
    let facts = tool_facts("set_tool_param");
    // The complete accepted set in `ProjectSession::set_tool_param`.
    for param in [
        "diameter",
        "flute_count",
        "stickout",
        "corner_radius",
        "cutting_length",
        "included_angle",
        "taper_half_angle",
        "shaft_diameter",
        "shank_diameter",
        "shank_length",
        "holder_diameter",
    ] {
        assert!(
            facts.description.contains(param),
            "set_tool_param's description omits the accepted param `{param}` — an agent \
             reading only the description concludes it is unsupported"
        );
    }
}

/// Gaps 1 + 2 — origin, material and rigidity are expressible, every
/// field is optional, and the description states the `auto_from_model`
/// rule. The silent disagreement between "what I set" and "what is
/// stored" is the actual defect, so the rule has to be on the surface.
#[test]
fn set_stock_config_carries_origin_material_and_rigidity() {
    let facts = tool_facts("set_stock_config");
    for field in [
        "x",
        "y",
        "z",
        "origin_x",
        "origin_y",
        "origin_z",
        "material",
        "workholding_rigidity",
        "auto_from_model",
    ] {
        let _ = property(&facts, field);
    }
    assert!(
        required(&facts).is_empty(),
        "set_stock_config must be an all-optional patch so material can be set on its own \
         without restating dimensions — required: {:?}",
        required(&facts)
    );
    assert!(
        facts.description.contains("auto_from_model"),
        "the description must state what happens to auto_from_model: {}",
        facts.description
    );
}

/// Gap 4 — the type-defining geometry is expressible AND required, and
/// tool numbers are addressable. `included_angle` cannot be optional in
/// spirit only: the description has to say the call is refused without
/// it, because that is what the handler does.
#[test]
fn add_tool_takes_per_type_geometry_and_a_tool_number() {
    let facts = tool_facts("add_tool");
    for field in [
        "included_angle",
        "taper_half_angle",
        "corner_radius",
        "flute_count",
        "cutting_length",
        "shaft_diameter",
        "shank_diameter",
        "shank_length",
        "stickout",
        "holder_diameter",
        "tool_number",
    ] {
        let _ = property(&facts, field);
    }
    let mut required = required(&facts);
    required.sort();
    assert_eq!(
        required,
        vec![
            "diameter".to_owned(),
            "name".to_owned(),
            "tool_type".to_owned()
        ],
        "the per-type geometry is conditionally required and enforced by the handler, not by \
         the schema — JSON Schema cannot express 'required iff tool_type == v_bit' here"
    );
    for phrase in [
        "included_angle",
        "taper_half_angle",
        "corner_radius",
        "tool_number",
    ] {
        assert!(
            facts.description.contains(phrase),
            "add_tool's description must mention `{phrase}`"
        );
    }
    assert!(
        facts.description.contains("defaulted"),
        "add_tool must tell the agent that unsupplied fields are reported in `defaulted`: {}",
        facts.description
    );
}

/// Gap 7 — both of these operations are valid (`parse_operation_type`
/// accepts them and its own error message lists them), but the tool
/// description omitted them, so an agent reading only the description
/// concludes the newest finishing operation is unreachable over MCP.
#[test]
fn add_toolpath_description_lists_every_operation_it_accepts() {
    let facts = tool_facts("add_toolpath");
    for op in rs_cam_core::compute::catalog::OperationType::ALL {
        assert!(
            facts.description.contains(op.kind_str()),
            "add_toolpath's description omits the valid operation `{}`",
            op.kind_str()
        );
    }
}

/// Gap 8 — a setup's in-plane rotation was in the data model with no
/// way to express it over MCP, so a rotated second setup could not be
/// authored at all.
#[test]
fn setup_rotation_has_a_setter() {
    let facts = tool_facts("set_setup_rotation");
    let _ = property(&facts, "setup_index");
    let _ = property(&facts, "z_rotation");
    for angle in ["90", "180", "270"] {
        assert!(
            facts.description.contains(angle),
            "the description must name the accepted angle {angle}"
        );
    }
}

/// Gap 3 — kinematics have a typed write path, not just a GRBL `$$`
/// paste. Acceleration decides the parallel-vs-spiral verdict, so this
/// is not cosmetic.
#[test]
fn machine_kinematics_have_a_typed_write_path() {
    let facts = tool_facts("set_machine_kinematics");
    for field in [
        "acceleration_x_mm_s2",
        "acceleration_y_mm_s2",
        "acceleration_z_mm_s2",
        "acceleration_mm_s2",
        "junction_deviation_mm",
    ] {
        let _ = property(&facts, field);
    }
    assert!(
        required(&facts).is_empty(),
        "set_machine_kinematics must be an all-optional patch"
    );
}
