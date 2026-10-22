//! Geometry selection P2.3b: the selection of the op decides what it cuts.
//!
//! # The claim
//!
//! A drawing holds two squares on two layers, `CUT` (left of X 0) and
//! `KEEP` (right of X 0). Through the session door
//! (`ProjectSession::generate_toolpath`):
//!
//! - a Pocket with the default selection cuts both squares;
//! - a Pocket whose selection is one rule on layer `CUT` cuts the `CUT`
//!   square only: no cutting move reaches the `KEEP` square;
//! - a rule on a layer that the drawing does not hold refuses and names the
//!   layer (S21), and no toolpath is stored.
//!
//! Before P2.3b the session resolved `Selection::drawing()` for every op,
//! whatever its config held.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_layer_rule_cuts_only_its_layer_p2_3
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::session::{polygon_model, single_op_session, stock_under};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{Filter, Item, ItemSet, Rule, Selection, Shape, SourceRef};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::MoveType;

/// The half side of each square (mm).
const HALF: f64 = 6.0;
/// The X of the centre of each square: `-GAP_X` (CUT) and `+GAP_X` (KEEP).
const GAP_X: f64 = 10.0;
/// The stock half extent and height (mm).
const STOCK_HALF: f64 = 20.0;
const STOCK_HEIGHT: f64 = 10.0;
/// The tool diameter (mm).
const TOOL_DIAMETER: f64 = 3.0;

fn square(cx: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(cx - HALF, -HALF),
        P2::new(cx + HALF, -HALF),
        P2::new(cx + HALF, HALF),
        P2::new(cx - HALF, HALF),
    ])
}

/// The two-layer drawing: `CUT` at `-GAP_X`, `KEEP` at `+GAP_X`.
fn drawing() -> ItemSet {
    let item = |layer: &str, cx: f64| {
        Item::new(
            SourceRef::ToolpathModel,
            Some(layer.into()),
            Shape::Loop(square(cx)),
        )
        .unwrap()
    };
    ItemSet::new(vec![item("CUT", -GAP_X), item("KEEP", GAP_X)])
}

fn layer_rule(layer: &str) -> Selection {
    Selection::from_rules(vec![Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec![layer.to_owned()],
            ..Filter::default()
        },
    }])
}

fn session(selection: Selection) -> ProjectSession {
    let mut model = polygon_model(Vec::new(), "p2_3_layers");
    model.items = Arc::new(drawing());
    single_op_session(
        stock_under(STOCK_HALF, STOCK_HEIGHT),
        endmill_tool_config(TOOL_DIAMETER),
        model,
        "layer pocket",
        OperationConfig::Pocket(PocketConfig {
            selection,
            ..PocketConfig::default()
        }),
    )
}

/// The X of every cutting move (linear and arc feeds).
fn cut_xs(session: &mut ProjectSession) -> Vec<f64> {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .expect("the pocket generates");
    let xs: Vec<f64> = result
        .toolpath()
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| m.target.x)
        .collect();
    assert!(!xs.is_empty(), "the pocket cut nothing");
    xs
}

#[test]
fn the_default_selection_cuts_both_layers() {
    let xs = cut_xs(&mut session(Selection::drawing()));
    assert!(xs.iter().any(|x| *x < 0.0), "the CUT square is not cut");
    assert!(xs.iter().any(|x| *x > 0.0), "the KEEP square is not cut");
}

#[test]
fn a_layer_rule_cuts_only_its_layer() {
    let xs = cut_xs(&mut session(layer_rule("CUT")));
    let right = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        right < 0.0,
        "a cutting move reached X {right}: the KEEP square (X > 0) was cut"
    );
}

#[test]
fn a_rule_on_an_absent_layer_refuses_and_names_it() {
    let mut s = session(layer_rule("NO_SUCH_LAYER"));
    let cancel = AtomicBool::new(false);
    let Err(err) = s.generate_toolpath(0, &cancel) else {
        panic!("a rule on an absent layer generated a toolpath");
    };
    let text = err.to_string();
    assert!(
        text.contains("NO_SUCH_LAYER"),
        "the refusal must name the layer: {text}"
    );
    assert!(s.result_at(0).is_none(), "the refusal stored a result");
}
