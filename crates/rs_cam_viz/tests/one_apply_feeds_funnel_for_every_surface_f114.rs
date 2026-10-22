//! F114 sentry — one Apply-Suggest funnel for the GUI, MCP and the CLI.
//!
//! Before F114 two funnels wrote a recommendation. The GUI modal and the
//! MCP `apply_feeds` tool ran `feeds::suggest::apply` with no stock slot
//! and no drill defaults. The CLI `--apply-suggest` ran
//! `suggest_for_operation`, which wrote the drill defaults, and stamped
//! the provenance by hand. A Drill or AlignmentPinDrill `peck_depth`
//! differed by surface.
//!
//! Now core's `Command::ApplyFeeds` is the one funnel:
//!
//! - the GUI and MCP door is `AppController::apply_feeds_recommendation`
//!   (the MCP arm `mcp_apply_feeds` calls it; `RsCamApp` needs an
//!   `eframe::CreationContext`, so no test drives the wire itself);
//! - the CLI applies `Command::ApplyFeeds` through `apply_command`, which
//!   is `ProjectSession::apply`.
//!
//! The claims:
//!
//! 1. The same project and the same scope give byte-equal configurations
//!    (operation and provenance, serialised) through both doors, for a
//!    Pocket, a Drill and an AlignmentPinDrill, under every scope.
//! 2. Under `Both` the Drill and the pin drill `peck_depth` move from the
//!    registry default (the drill defaults run on the GUI door now). Under
//!    `Speeds` the peck stays: the peck is cut geometry.
//! 3. The pin drill peck is capped by the stock depth, so the stock slot
//!    reaches the funnel.
//! 4. A source scan: the CLI and the MCP arm reach the one funnel, and no
//!    GUI or CLI source calls `suggest::apply` itself.
//!
//! Red proof (injected 2026-10-21): with the drill-defaults call in
//! `feeds::suggest::apply` switched off, claim 2 fails on the Drill row
//! (the peck stays at the registry default 3 mm), and claim 1 fails its
//! non-vacuity guard (`CutGeometry` writes nothing on the Drill).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::ApplyScope;
use rs_cam_core::session::{
    ApplyFeedsArgs, Command, ProjectSession, ProjectSessionBuilder, ToolpathConfig,
};
use rs_cam_viz::controller::AppController;

/// The stock thickness, mm. The pin drill hole is this plus the
/// spoilboard penetration, so the cap in claim 3 reads it.
const STOCK_Z_MM: f64 = 12.0;

fn toolpath(name: &str, op_type: OperationType) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(op_type),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(1),
        model_id: rs_cam_core::ids::ModelId::from_raw(0),
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        rest_analysis: Default::default(),
        stock_source: StockSource::Fresh,
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        planner_origin: None,
    }
}

/// The ops of the fixture, in toolpath order.
const OPS: [(&str, OperationType); 3] = [
    ("Pocket", OperationType::Pocket),
    ("Drill", OperationType::Drill),
    ("Pin drill", OperationType::AlignmentPinDrill),
];

fn session() -> ProjectSession {
    let tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    let stock = StockConfig {
        z: STOCK_Z_MM,
        ..StockConfig::default()
    };
    let mut builder = ProjectSessionBuilder::new().tool(tool).stock(stock);
    for (name, op_type) in OPS {
        let _ = builder
            .add_toolpath(0, toolpath(name, op_type))
            .expect("add the toolpath");
    }
    builder.build()
}

/// The written configuration of toolpath `index`, as bytes.
fn written(session: &ProjectSession, index: usize) -> String {
    let tc = &session.toolpath_configs()[index];
    serde_json::to_string(&(&tc.operation, &tc.feeds_provenance)).expect("serialise")
}

fn peck(op: &OperationConfig) -> Option<f64> {
    match op {
        OperationConfig::Drill(cfg) => Some(cfg.peck_depth),
        OperationConfig::AlignmentPinDrill(cfg) => Some(cfg.peck_depth),
        _ => None,
    }
}

/// Apply `scope` to every toolpath through the GUI and MCP door.
fn through_the_controller(scope: ApplyScope) -> ProjectSession {
    let mut controller = AppController::new();
    controller.state.replace_session(session());
    let ids: Vec<_> = controller
        .state
        .session()
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    for id in ids {
        let _ = controller
            .apply_feeds_recommendation(id, scope)
            .expect("a flat end mill runs every fixture op");
    }
    controller.state.session().clone()
}

/// Apply `scope` to every toolpath as the CLI does: the core row.
fn through_the_core_row(scope: ApplyScope) -> ProjectSession {
    let mut s = session();
    for index in 0..s.toolpath_configs().len() {
        let _ = s
            .apply(Command::ApplyFeeds(ApplyFeedsArgs {
                index,
                scope,
                explored: None,
            }))
            .expect("a flat end mill runs every fixture op");
    }
    s
}

/// Claim 1: byte-equal configurations through both doors, every scope.
#[test]
fn every_surface_writes_the_same_configuration_f114() {
    for scope in [
        ApplyScope::Both,
        ApplyScope::Speeds,
        ApplyScope::CutGeometry,
    ] {
        let gui = through_the_controller(scope);
        let core = through_the_core_row(scope);
        let fresh = session();
        for (index, (name, _)) in OPS.iter().enumerate() {
            assert_ne!(
                written(&core, index),
                written(&fresh, index),
                "fixture: {scope:?} wrote nothing on {name}, so the parity proves nothing"
            );
            assert_eq!(
                written(&gui, index),
                written(&core, index),
                "{scope:?} on {name}: the GUI/MCP door and the core row (the CLI) \
                 must write the same configuration"
            );
        }
    }
}

/// Claims 2 and 3: the drill defaults run on the one funnel, under a
/// geometry scope only, and the pin peck reads the stock.
#[test]
fn the_drill_defaults_reach_every_surface_f114() {
    let fresh = session();
    let both = through_the_controller(ApplyScope::Both);
    let speeds = through_the_controller(ApplyScope::Speeds);
    let mut moved = 0_usize;
    for (index, (name, op_type)) in OPS.iter().enumerate() {
        let Some(default_peck) = peck(&fresh.toolpath_configs()[index].operation) else {
            continue;
        };
        let applied = peck(&both.toolpath_configs()[index].operation).unwrap();
        assert!(
            (applied - default_peck).abs() > 1e-9,
            "{name}: Apply all left the peck at the registry default {default_peck}; \
             the drill defaults did not run on the GUI door"
        );
        let kept = peck(&speeds.toolpath_configs()[index].operation).unwrap();
        assert!(
            (kept - default_peck).abs() < 1e-12,
            "{name}: a speeds apply moved the peck {default_peck} -> {kept}; the peck is \
             cut geometry"
        );
        if *op_type == OperationType::AlignmentPinDrill {
            let OperationConfig::AlignmentPinDrill(cfg) = &both.toolpath_configs()[index].operation
            else {
                unreachable!("the row is a pin drill");
            };
            let hole = STOCK_Z_MM + cfg.spoilboard_penetration;
            // The peck the drill defaults give with no stock slot.
            let mut uncapped = fresh.toolpath_configs()[index].operation.clone();
            rs_cam_core::feeds::suggest::apply_drill_defaults(
                &mut uncapped,
                &fresh.tools()[0],
                &fresh.stock_config().material,
                None,
            );
            let uncapped = peck(&uncapped).unwrap();
            assert!(
                uncapped > hole,
                "fixture: the uncapped peck {uncapped} must exceed the {hole} mm hole, \
                 or the cap proves nothing"
            );
            assert!(
                applied <= hole,
                "{name}: peck {applied} is not capped by the {hole} mm hole; the stock \
                 slot did not reach the funnel"
            );
        }
        moved += 1;
    }
    assert_eq!(
        moved, 2,
        "fixture: the Drill and the pin drill rows must both be read"
    );
}

/// Strip `//` comments so the scan reads code only.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

fn source(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The body of `fn name` in `src`, to the next `fn ` at the same indent.
fn body<'a>(src: &'a str, signature: &str) -> &'a str {
    let start = src
        .find(signature)
        .unwrap_or_else(|| panic!("anchor `{signature}` is gone; the scan reads nothing"));
    let rest = &src[start + signature.len()..];
    let end = rest
        .find("\n    fn ")
        .or_else(|| rest.find("\nfn "))
        .unwrap_or(rest.len());
    &rest[..end]
}

/// Claim 4: the CLI and the MCP arm reach the one funnel.
#[test]
fn every_surface_reaches_the_core_row_f114() {
    let cli = code_only(&source("../rs_cam_cli/src/project.rs"));
    let cli_fn = body(&cli, "fn apply_suggested_feeds_to_session(");
    assert!(
        cli_fn.contains("Command::ApplyFeeds"),
        "the CLI --apply-suggest must apply core's Command::ApplyFeeds"
    );
    let mcp = code_only(&source("src/app/mcp/generation.rs"));
    let mcp_fn = body(&mcp, "fn mcp_apply_feeds(");
    assert!(
        mcp_fn.contains("apply_feeds_recommendation"),
        "the MCP apply_feeds arm must take the controller funnel"
    );
    let events = code_only(&source("src/controller/events/mod.rs"));
    let funnel = body(&events, "fn apply_feeds_through_funnel(");
    assert!(
        funnel.contains("Command::ApplyFeeds"),
        "the controller funnel must apply core's Command::ApplyFeeds"
    );
    for (file, text) in [("cli project.rs", &cli), ("controller events", &events)] {
        assert!(
            !text.contains("suggest::apply("),
            "{file} calls feeds::suggest::apply itself; a second funnel is back"
        );
    }
}
