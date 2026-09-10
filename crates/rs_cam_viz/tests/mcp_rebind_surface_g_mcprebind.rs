//! G-MCPREBIND, MCP half — the embedded server registers a route to a
//! toolpath's TOOL and INPUT MODEL bindings, and its schema says which
//! number it wants.
//!
//! Tasks F3.7 / F3.8 (`planning/ui_fix_2026-09-09/PLAN.md` §5), specified
//! by `research/R0.3.md` §4 and its §7 Q4: "MCP has no tool to rebind a
//! toolpath's tool or model … So on MCP 'the op exists, fix the binding'
//! has no fix path today."
//!
//! Pre-fix, `EmbeddedCamServer::into_tool_router()` registered neither
//! name, and the only key an agent could try —
//! `set_toolpath_param(index, "tool_id", …)` — was refused (the core half
//! of this sentry, `rs_cam_core/tests/toolpath_rebind_g_mcprebind.rs`,
//! carries the verbatim refusal).
//!
//! What is asserted here is the schema and the description text the LLM
//! actually receives, in the style of `mcp_authoring_surface.rs`. On this
//! surface an inaccurate description is a real defect: `add_toolpath`
//! takes a tool **index** and these take a tool **id**, and the two agree
//! in every project that has never had a tool removed — so an agent that
//! guesses wrong is not told, it is silently obeyed.

#![cfg(feature = "mcp")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_viz::mcp_server::EmbeddedCamServer;

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
        .unwrap_or_else(|| panic!("the embedded server does not register `{name}`"));
    ToolFacts {
        description: tool.description.map(|d| d.into_owned()).unwrap_or_default(),
        schema: serde_json::Value::Object((*tool.input_schema).clone()),
    }
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

fn property<'a>(facts: &'a ToolFacts, field: &str) -> &'a serde_json::Value {
    facts
        .schema
        .pointer(&format!("/properties/{field}"))
        .unwrap_or_else(|| panic!("missing property `{field}` in {}", facts.schema))
}

/// F3.7 / F3.8 — the routes exist at all. This is the assertion that
/// fails on the pre-fix tree: `tool_facts` panics with "the embedded
/// server does not register `set_toolpath_tool`".
#[test]
fn the_two_rebind_routes_are_registered() {
    for name in ["set_toolpath_tool", "set_toolpath_model"] {
        let facts = tool_facts(name);
        assert!(
            !facts.description.is_empty(),
            "{name} must carry a description — it is the only thing an \
             agent reads before choosing a tool"
        );
    }
}

#[test]
fn set_toolpath_tool_takes_an_index_and_a_tool_id_and_both_are_required() {
    let facts = tool_facts("set_toolpath_tool");
    let req = required(&facts);
    for field in ["index", "tool_id"] {
        assert!(
            req.contains(&field.to_owned()),
            "set_toolpath_tool must require `{field}` — got {req:?}"
        );
        let ty = property(&facts, field).pointer("/type").cloned();
        assert_eq!(
            ty,
            Some(serde_json::json!("integer")),
            "`{field}` must be an integer, got {ty:?}"
        );
    }
    assert!(
        !req.contains(&"tool_index".to_owned()),
        "the field is named `tool_id` on purpose; `tool_index` would read \
         as add_toolpath's positional argument"
    );
}

#[test]
fn set_toolpath_model_takes_an_index_and_a_model_id_and_both_are_required() {
    let facts = tool_facts("set_toolpath_model");
    let req = required(&facts);
    for field in ["index", "model_id"] {
        assert!(
            req.contains(&field.to_owned()),
            "set_toolpath_model must require `{field}` — got {req:?}"
        );
        let ty = property(&facts, field).pointer("/type").cloned();
        assert_eq!(ty, Some(serde_json::json!("integer")));
    }
}

/// The id-vs-index trap, stated where the agent reads it. `add_toolpath`
/// takes a positional `tool_index`; this takes a project-assigned id.
#[test]
fn the_descriptions_say_id_not_index() {
    let tool = tool_facts("set_toolpath_tool");
    let tool_text = format!(
        "{} {}",
        tool.description,
        property(&tool, "tool_id")
            .pointer("/description")
            .and_then(|d| d.as_str())
            .unwrap_or_default()
    );
    assert!(
        tool_text.contains("list_tools"),
        "set_toolpath_tool must name where the id comes from: {tool_text}"
    );
    assert!(
        tool_text.contains("NOT") && tool_text.contains("add_toolpath"),
        "set_toolpath_tool must say the number is not the positional index \
         `add_toolpath` takes: {tool_text}"
    );

    let model = tool_facts("set_toolpath_model");
    let model_text = format!(
        "{} {}",
        model.description,
        property(&model, "model_id")
            .pointer("/description")
            .and_then(|d| d.as_str())
            .unwrap_or_default()
    );
    assert!(
        model_text.contains("inspect_model"),
        "set_toolpath_model must name where the id comes from: {model_text}"
    );
    assert!(
        model_text.contains("NOT"),
        "set_toolpath_model must say the number is not a positional index: \
         {model_text}"
    );
}

/// Three facts the descriptions must carry, because each is a thing an
/// agent would otherwise have to discover by breaking a job:
///
/// 1. the rebind invalidates and needs a regenerate;
/// 2. `set_toolpath_param` is NOT the route (its refusal is the pre-fix
///    dead end this task closes);
/// 3. a tool the operation's shape constraint rejects can still be bound
///    — that is what makes the blocked op repairable rather than a dead
///    end, per R0.3 §3.1.
#[test]
fn the_rebind_descriptions_state_their_consequences() {
    let tool = tool_facts("set_toolpath_tool");
    assert!(
        tool.description.contains("Regenerate") || tool.description.contains("regenerate to apply"),
        "set_toolpath_tool must say a regenerate is needed: {}",
        tool.description
    );
    assert!(
        tool.description.contains("set_toolpath_param"),
        "set_toolpath_tool must say the param route does NOT reach the \
         tool binding: {}",
        tool.description
    );
    assert!(
        tool.description.contains("Scallop"),
        "set_toolpath_tool must name the blocked-op repair case: {}",
        tool.description
    );

    let model = tool_facts("set_toolpath_model");
    assert!(
        model.description.contains("regenerate") || model.description.contains("Regenerate"),
        "set_toolpath_model must say a regenerate is needed: {}",
        model.description
    );
    assert!(
        model.description.contains("face selection"),
        "set_toolpath_model must warn that BREP face ids belong to the \
         model that was bound when they were picked: {}",
        model.description
    );
}

/// `set_toolpath_param` keeps its meaning: operation params, nothing
/// else. Its description must not offer either binding key as a param —
/// the core router refuses both — and it must send the reader to the
/// route that does work, because an agent that reads only this
/// description is exactly the agent that hits the dead end.
#[test]
fn set_toolpath_param_does_not_claim_to_rebind_and_names_the_route_that_does() {
    let facts = tool_facts("set_toolpath_param");
    for forbidden in ["tool_id", "model_id"] {
        assert!(
            !facts.description.contains(forbidden),
            "set_toolpath_param's description must not offer `{forbidden}` — \
             the core router refuses that key: {}",
            facts.description
        );
    }
    for pointer in ["set_toolpath_tool", "set_toolpath_model"] {
        assert!(
            facts.description.contains(pointer),
            "set_toolpath_param must point at `{pointer}`: {}",
            facts.description
        );
    }
}
