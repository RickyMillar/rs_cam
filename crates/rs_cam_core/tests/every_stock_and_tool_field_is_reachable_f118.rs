//! F118 (tech-debt review 2026-10-19): the field-level Reach census.
//!
//! The registry says `set_stock_config` and `set_tool_param` are
//! `mcp: Reach::Reached`. Before F118 that was true only of some fields:
//! the wire could not set `padding`, `helix_deg`, `corner_radius_mm`,
//! `tool_material` or `cut_direction`. This sentry holds that every field
//! of [`StockConfig`] and [`ToolConfig`] is one of these two kinds:
//!
//! - a field that the wire patch writes (a [`StockConfigPatch`] field, or a
//!   [`ToolParamKey`] token);
//! - a field in the `OWN_DOOR` list of this file, with the door that writes
//!   it.
//!
//! Each struct is destructured with no `..`, so a new field does not
//! compile here until someone puts it in one of the two kinds. The MCP
//! wire reaches the patch through an exhaustive `From` in `rs_cam_mcp`, so
//! the wire and the patch cannot differ either.

#![allow(clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::stock_config::{StockConfig, StockConfigPatch};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolParamKey, ToolType};

/// The stock fields that the patch does not write, and the door of each.
const STOCK_OWN_DOOR: &[(&str, &str)] = &[
    (
        "alignment_pins",
        "set_alignment_pins / add_alignment_pin / remove_alignment_pin",
    ),
    ("pin_part_models", "SetPinPartModels (the stock panel)"),
];

/// The tool fields that `set_tool_param` does not write, and the door of each.
const TOOL_OWN_DOOR: &[(&str, &str)] = &[
    ("id", "the session assigns it at add_tool"),
    ("name", "ReplaceTool (the GUI tool panel) and add_tool"),
    ("tool_number", "ReplaceTool and add_tool (tool_number)"),
    (
        "tool_type",
        "ReplaceTool and add_tool: a new type is a new cutter",
    ),
    ("vendor", "ReplaceTool and add_tool_from_library"),
    ("product_id", "ReplaceTool and add_tool_from_library"),
];

/// Every field name of [`StockConfig`], by an exhaustive destructure.
fn stock_fields() -> Vec<&'static str> {
    let StockConfig {
        x: _,
        y: _,
        z: _,
        origin_x: _,
        origin_y: _,
        origin_z: _,
        auto_from_model: _,
        padding: _,
        material: _,
        alignment_pins: _,
        pin_part_models: _,
    } = StockConfig::default();
    vec![
        "x",
        "y",
        "z",
        "origin_x",
        "origin_y",
        "origin_z",
        "auto_from_model",
        "padding",
        "material",
        "alignment_pins",
        "pin_part_models",
    ]
}

/// Every field name of [`StockConfigPatch`], by an exhaustive destructure.
fn patch_fields() -> Vec<&'static str> {
    let StockConfigPatch {
        x: _,
        y: _,
        z: _,
        origin_x: _,
        origin_y: _,
        origin_z: _,
        padding: _,
        material: _,
        auto_from_model: _,
    } = StockConfigPatch::default();
    vec![
        "x",
        "y",
        "z",
        "origin_x",
        "origin_y",
        "origin_z",
        "padding",
        "material",
        "auto_from_model",
    ]
}

/// Every field name of [`ToolConfig`], by an exhaustive destructure.
fn tool_fields() -> Vec<&'static str> {
    let ToolConfig {
        id: _,
        name: _,
        tool_number: _,
        tool_type: _,
        diameter: _,
        cutting_length: _,
        helix_deg: _,
        corner_radius_mm: _,
        corner_radius: _,
        included_angle: _,
        taper_half_angle: _,
        shaft_diameter: _,
        shank_diameter: _,
        stickout: _,
        flute_count: _,
        tool_material: _,
        cut_direction: _,
        vendor: _,
        product_id: _,
        size_units: _,
    } = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    vec![
        "id",
        "name",
        "tool_number",
        "tool_type",
        "diameter",
        "cutting_length",
        "helix_deg",
        "corner_radius_mm",
        "corner_radius",
        "included_angle",
        "taper_half_angle",
        "shaft_diameter",
        "shank_diameter",
        "stickout",
        "flute_count",
        "tool_material",
        "cut_direction",
        "vendor",
        "product_id",
        "size_units",
    ]
}

/// The name lists above are hand-written beside the destructure. Each
/// name must be a key of the serialised struct, so a typo cannot hide a
/// field. `skip_serializing_if` hides an absent `Option`, so the probe
/// sets every `Option` first.
fn assert_names_are_serde_keys(names: &[&str], json: &serde_json::Value, what: &str) {
    let object = json.as_object().expect("a struct serialises to an object");
    for name in names {
        assert!(object.contains_key(*name), "{what}: `{name}` is no field");
    }
    assert_eq!(object.len(), names.len(), "{what}: the name list is short");
}

#[test]
fn every_stock_field_is_patched_or_has_its_own_door() {
    let stock_names = stock_fields();
    let probe = StockConfig {
        pin_part_models: Some(Vec::new()),
        ..StockConfig::default()
    };
    assert_names_are_serde_keys(
        &stock_names,
        &serde_json::to_value(&probe).expect("serialise the stock"),
        "StockConfig",
    );

    let patch = patch_fields();
    for name in &stock_names {
        let patched = patch.contains(name);
        let own_door = STOCK_OWN_DOOR.iter().any(|(n, _)| n == name);
        assert!(
            patched ^ own_door,
            "stock field `{name}`: patched = {patched}, own door = {own_door}. \
             Put it in exactly one of StockConfigPatch or STOCK_OWN_DOOR."
        );
    }
    for name in &patch {
        assert!(
            stock_names.contains(name),
            "the patch field `{name}` names no stock field"
        );
    }
}

#[test]
fn every_tool_field_is_a_param_key_or_has_its_own_door() {
    let tool_names = tool_fields();
    let mut probe = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    probe.size_units = Some(rs_cam_core::compute::tool_config::SizeUnits::Metric);
    assert_names_are_serde_keys(
        &tool_names,
        &serde_json::to_value(&probe).expect("serialise the tool"),
        "ToolConfig",
    );

    for name in &tool_names {
        let keyed = ToolParamKey::parse(name).is_some();
        let own_door = TOOL_OWN_DOOR.iter().any(|(n, _)| n == name);
        assert!(
            keyed ^ own_door,
            "tool field `{name}`: a ToolParamKey = {keyed}, own door = {own_door}. \
             Put it in exactly one of ToolParamKey or TOOL_OWN_DOOR."
        );
    }
    for key in ToolParamKey::ALL {
        assert!(
            tool_names.contains(&key.token()),
            "the key `{}` names no tool field",
            key.token()
        );
    }
}
