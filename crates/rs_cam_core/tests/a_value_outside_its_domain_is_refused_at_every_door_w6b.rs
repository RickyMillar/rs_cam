//! W6b (F030, F036, F210): a value outside its domain is refused at every
//! door, with a message that names the entity, the field, the value and
//! the domain.
//!
//! Before W6b only `set_toolpath_param` read the declared `#[param]`
//! ranges. `AddToolpath`, the op change and project load wrote any number,
//! a slope band could be inverted (it admits nothing), and the stock, the
//! machine and the shoe took their domain only from GUI drag literals.
//!
//! Each test drives one door through `ProjectSession::apply` (or the
//! loader) with one bad value and one good value (the positive control).
//! The builder is raw (it checks nothing), so it writes the bad records
//! that the loader tests save and load.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;
use common::make_endmill_6mm;
use common::session::toolpath_config;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::domain::{DomainRefusal, OpDomainRefusal};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::stock_domain::StockDimension;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::domain::MachineDimension;
use rs_cam_core::machine::head::{MachineHead, ShoeSetting};
use rs_cam_core::session::{
    AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder, SessionError,
    SetMachineHeadArgs, SetToolpathOperationArgs, SetToolpathParamArgs, SetToolpathShoeArgs,
    ToolpathConfig,
};
use serde_json::json;

fn op(op_type: OperationType) -> OperationConfig {
    OperationConfig::new_default(op_type)
}

fn zero_feed(op_type: OperationType) -> OperationConfig {
    let mut config = op(op_type);
    config.set_feed_rate(0.0);
    config
}

fn inverted_slope_band() -> OperationConfig {
    let mut config = op(OperationType::DropCutter);
    let OperationConfig::DropCutter(c) = &mut config else {
        panic!("the default of the op type")
    };
    c.slope_from = 60.0;
    c.slope_to = 30.0;
    config
}

fn inverted_thresholds() -> OperationConfig {
    let mut config = op(OperationType::UnifiedFinish);
    let OperationConfig::UnifiedFinish(c) = &mut config else {
        panic!("the default of the op type")
    };
    c.steep_threshold_deg = 80.0;
    c.waterline_threshold_deg = 60.0;
    config
}

fn pocket_with_floor(floor: Floor) -> OperationConfig {
    let mut config = op(OperationType::Pocket);
    let OperationConfig::Pocket(c) = &mut config else {
        panic!("the default of the op type")
    };
    c.floor = floor;
    config
}

fn tp(config: OperationConfig) -> ToolpathConfig {
    toolpath_config(
        "w6b",
        config,
        rs_cam_core::ids::ToolId::from_raw(0),
        rs_cam_core::ids::ModelId::from_raw(0),
    )
}

/// A session with one tool, one setup and one valid Pocket.
fn session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    let _ = builder.add_tool(make_endmill_6mm());
    let _ = builder
        .add_toolpath(0, tp(op(OperationType::Pocket)))
        .unwrap();
    builder.build()
}

fn add(session: &mut ProjectSession, config: ToolpathConfig) -> Result<(), SessionError> {
    session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(config),
        }))
        .map(|_| ())
}

fn op_refusal(result: Result<(), SessionError>) -> OpDomainRefusal {
    match result {
        Err(SessionError::OutOfDomain(DomainRefusal::Operation { refusal, .. })) => refusal,
        other => panic!("expected an operation domain refusal, got {other:?}"),
    }
}

fn assert_feed_refused(result: Result<(), SessionError>) {
    let refusal = op_refusal(result);
    let text = refusal.to_string();
    assert!(
        matches!(&refusal, OpDomainRefusal::OutOfRange { param, value, .. }
            if param == "feed_rate" && *value == 0.0),
        "{text}"
    );
    assert!(text.contains("outside the accepted range"), "{text}");
}

fn assert_inverted(result: Result<(), SessionError>, low: &str, high: &str) {
    match op_refusal(result) {
        OpDomainRefusal::InvertedPair {
            low: l, high: h, ..
        } => {
            assert_eq!((l, h), (low, high));
        }
        other => panic!("expected an inverted pair, got {other}"),
    }
}

#[test]
fn add_toolpath_refuses_each_rule_and_accepts_a_good_config() {
    let mut s = session();
    let before = s.toolpath_count();
    assert_feed_refused(add(&mut s, tp(zero_feed(OperationType::Pocket))));
    assert_feed_refused(add(&mut s, tp(zero_feed(OperationType::Scallop))));
    assert_inverted(
        add(&mut s, tp(inverted_slope_band())),
        "slope_from",
        "slope_to",
    );
    assert_inverted(
        add(&mut s, tp(inverted_thresholds())),
        "steep_threshold_deg",
        "waterline_threshold_deg",
    );
    assert!(matches!(
        op_refusal(add(
            &mut s,
            tp(pocket_with_floor(Floor::Top { depth: 0.0 }))
        )),
        OpDomainRefusal::Floor { .. }
    ));
    let mut bad_shoe = tp(op(OperationType::Pocket));
    bad_shoe.shoe = ShoeSetting::Gap { gap_mm: -1.0 };
    assert!(matches!(
        add(&mut s, bad_shoe),
        Err(SessionError::OutOfDomain(DomainRefusal::Shoe { .. }))
    ));
    assert_eq!(s.toolpath_count(), before, "nothing added");

    // Positive controls: a good op, an equal slope band, and a signed
    // depth on a floor reference other than Top (a skin above the stock
    // bottom).
    add(&mut s, tp(op(OperationType::Scallop))).unwrap();
    let mut equal_band = inverted_slope_band();
    if let OperationConfig::DropCutter(c) = &mut equal_band {
        c.slope_to = c.slope_from;
    }
    add(&mut s, tp(equal_band)).unwrap();
    add(
        &mut s,
        tp(pocket_with_floor(Floor::StockBottom { depth: -0.5 })),
    )
    .unwrap();
    assert_eq!(s.toolpath_count(), before + 3);
}

#[test]
fn the_op_change_refuses_and_keeps_the_stored_op() {
    let mut s = session();
    let change = |s: &mut ProjectSession, operation: OperationConfig| {
        s.apply(Command::SetToolpathOperation(SetToolpathOperationArgs {
            index: 0,
            operation: Box::new(operation),
        }))
        .map(|_| ())
    };
    assert_feed_refused(change(&mut s, zero_feed(OperationType::Profile)));
    assert_inverted(
        change(&mut s, inverted_slope_band()),
        "slope_from",
        "slope_to",
    );
    assert_eq!(
        s.toolpath_configs()[0].operation.op_type(),
        OperationType::Pocket,
        "nothing written"
    );
    change(&mut s, op(OperationType::Profile)).unwrap();
    assert_eq!(
        s.toolpath_configs()[0].operation.op_type(),
        OperationType::Profile
    );
}

#[test]
fn the_param_setter_refuses_an_inverted_band() {
    let mut s = session();
    add(&mut s, tp(op(OperationType::RampFinish))).unwrap();
    let index = s.toolpath_count() - 1;
    let set = |s: &mut ProjectSession, param: &str, value: f64| {
        s.apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index,
            param: param.to_owned(),
            value: json!(value),
        }))
        .map(|_| ())
    };
    set(&mut s, "slope_to", 50.0).unwrap();
    assert_inverted(set(&mut s, "slope_from", 70.0), "slope_from", "slope_to");
    set(&mut s, "slope_from", 40.0).unwrap();
    // The single-value range rule keeps its text (DR-LIVE).
    let text = set(&mut s, "slope_from", 95.0).unwrap_err().to_string();
    assert!(text.contains("outside the accepted range"), "{text}");
}

#[test]
fn the_head_and_shoe_doors_refuse() {
    let mut s = session();
    let head = MachineHead {
        mount_mm: -1.0,
        ..MachineHead::default()
    };
    assert!(matches!(
        s.apply(Command::SetMachineHead(SetMachineHeadArgs {
            head: Box::new(head)
        })),
        Err(SessionError::OutOfDomain(DomainRefusal::Head(_)))
    ));
    let shoe = |s: &mut ProjectSession, gap_mm: f64| {
        s.apply(Command::SetToolpathShoe(SetToolpathShoeArgs {
            index: 0,
            shoe: ShoeSetting::Gap { gap_mm },
        }))
        .map(|_| ())
    };
    assert!(matches!(
        shoe(&mut s, f64::NAN),
        Err(SessionError::OutOfDomain(DomainRefusal::Shoe { .. }))
    ));
    assert_eq!(s.toolpath_configs()[0].shoe, ShoeSetting::Auto);
    shoe(&mut s, 0.0).unwrap();
}

/// Save a session the raw builder made, then load it.
fn round_trip(name: &str, session: &ProjectSession) -> Result<ProjectSession, SessionError> {
    let dir = std::env::temp_dir().join(format!("rs_cam_w6b_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("project.toml");
    session.save(&path).unwrap();
    let loaded = ProjectSession::load(&path);
    let _ = std::fs::remove_dir_all(&dir);
    loaded
}

fn raw(stock: StockConfig, machine: MachineProfile, config: ToolpathConfig) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock).machine(machine);
    let _ = builder.add_tool(make_endmill_6mm());
    let _ = builder.add_toolpath(0, config).unwrap();
    builder.build()
}

#[test]
fn the_loader_refuses_each_entity_and_names_the_field() {
    let good = || {
        (
            StockConfig::default(),
            MachineProfile::default(),
            tp(op(OperationType::Pocket)),
        )
    };
    // Positive control.
    let (stock, machine, config) = good();
    round_trip("good", &raw(stock, machine, config)).unwrap();

    let (stock, machine, _) = good();
    let err = round_trip(
        "op",
        &raw(stock, machine, tp(zero_feed(OperationType::Pocket))),
    )
    .err()
    .expect("a zero feed refuses at load");
    let text = err.to_string();
    assert!(
        text.contains("w6b") && text.contains("feed_rate") && text.contains("= 0"),
        "{text}"
    );

    let (stock, machine, _) = good();
    assert!(matches!(
        round_trip("band", &raw(stock, machine, tp(inverted_thresholds()))),
        Err(SessionError::OutOfDomain(DomainRefusal::Operation {
            refusal: OpDomainRefusal::InvertedPair { .. },
            ..
        }))
    ));

    let (mut stock, machine, config) = good();
    stock.z = 0.0;
    match round_trip("stock", &raw(stock, machine, config)) {
        Err(SessionError::OutOfDomain(DomainRefusal::Stock(e))) => {
            assert_eq!(e.dimension, StockDimension::Z);
        }
        other => panic!("expected a stock refusal, got {:?}", other.err()),
    }

    let (stock, mut machine, config) = good();
    machine.max_feed_mm_min = 0.0;
    match round_trip("machine", &raw(stock, machine, config)) {
        Err(SessionError::OutOfDomain(DomainRefusal::Machine(e))) => {
            assert_eq!(e.dimension, MachineDimension::MaxFeed);
        }
        other => panic!("expected a machine refusal, got {:?}", other.err()),
    }

    let (stock, mut machine, config) = good();
    machine.head.mount_mm = -2.0;
    assert!(matches!(
        round_trip("head", &raw(stock, machine, config)),
        Err(SessionError::OutOfDomain(DomainRefusal::Head(_)))
    ));

    let (stock, machine, mut config) = good();
    config.shoe = ShoeSetting::Gap { gap_mm: -3.0 };
    assert!(matches!(
        round_trip("shoe", &raw(stock, machine, config)),
        Err(SessionError::OutOfDomain(DomainRefusal::Shoe { .. }))
    ));
}

// ── The doors whose command arm is in `session/command.rs` ──────────

use rs_cam_core::session::{ReplaceToolpathConfigArgs, SetMachineArgs, SetStockConfigArgs};

#[test]
fn replace_toolpath_config_refuses() {
    let mut s = session();
    let result = s
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: 0,
            config: Box::new(tp(zero_feed(OperationType::Pocket))),
        }))
        .map(|_| ());
    assert_feed_refused(result);
}

#[test]
fn set_stock_config_refuses() {
    let mut s = session();
    let stock = StockConfig {
        x: -1.0,
        ..StockConfig::default()
    };
    assert!(matches!(
        s.apply(Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(stock)
        })),
        Err(SessionError::OutOfDomain(DomainRefusal::Stock(_)))
    ));
}

#[test]
fn set_machine_refuses() {
    let mut s = session();
    let machine = MachineProfile {
        aggressiveness: 0.0,
        ..MachineProfile::default()
    };
    assert!(matches!(
        s.apply(Command::SetMachine(SetMachineArgs {
            machine: Box::new(machine)
        })),
        Err(SessionError::OutOfDomain(DomainRefusal::Machine(_)))
    ));
}
