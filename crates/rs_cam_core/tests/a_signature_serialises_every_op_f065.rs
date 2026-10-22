//! F065: a change-detection signature never folds a serialise failure to an
//! empty string.
//!
//! `ToolpathConfig::generation_inputs_signature` and
//! `compute::simulate::hash_operation_config` serialise the operation (and,
//! for the signature, the dressups and the heights) with serde-json. They
//! used to fold a failure to an empty value, so two different configs could
//! read as equal and keep a stale result. They now `expect` the serialise,
//! on the claim that these types hold no map and so cannot be refused.
//! This file holds that claim for the default of every operation type: each
//! one signs, each signature holds the operation text, and no two operation
//! types sign equal.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_signature_serialises_every_op_f065
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeMap;

use common::session::toolpath_config;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::simulate::hash_operation_config;

#[test]
fn every_op_default_signs_and_no_two_ops_sign_equal() {
    assert!(
        OperationType::ALL.len() > 1,
        "non-vacuity: the catalogue lists more than one operation type"
    );
    let mut by_signature: BTreeMap<String, OperationType> = BTreeMap::new();
    let mut by_hash: BTreeMap<u64, OperationType> = BTreeMap::new();
    for &op_type in OperationType::ALL {
        let op = OperationConfig::new_default(op_type);
        let operation_json = serde_json::to_string(&op).expect("the op serialises");
        assert!(!operation_json.is_empty(), "{op_type:?}: empty op text");

        let tc = toolpath_config(
            "f065",
            op.clone(),
            rs_cam_core::ids::ToolId::from_raw(0),
            rs_cam_core::ids::ModelId::from_raw(0),
        );
        let signature = tc.generation_inputs_signature();
        assert!(
            signature.starts_with(&operation_json),
            "{op_type:?}: the signature must open with the op text"
        );
        if let Some(other) = by_signature.insert(signature, op_type) {
            panic!("{op_type:?} and {other:?} sign equal");
        }
        if let Some(other) = by_hash.insert(hash_operation_config(&op), op_type) {
            panic!("{op_type:?} and {other:?} hash equal");
        }
    }
}

/// A field outside the signature moves nothing; a generation input moves
/// it. The destructure in `generation_inputs_signature` names each field
/// on one side.
#[test]
fn a_rename_keeps_the_signature_and_a_tool_change_moves_it() {
    let op = OperationConfig::new_default(OperationType::ALL[0]);
    let base = toolpath_config(
        "f065",
        op,
        rs_cam_core::ids::ToolId::from_raw(0),
        rs_cam_core::ids::ModelId::from_raw(0),
    );
    let mut renamed = base.clone();
    renamed.name = "renamed".to_owned();
    renamed.boundary_inherit = !renamed.boundary_inherit;
    assert_eq!(
        base.generation_inputs_signature(),
        renamed.generation_inputs_signature()
    );
    let mut retooled = base.clone();
    retooled.tool_id = rs_cam_core::ids::ToolId::from_raw(retooled.tool_id.raw() + 1);
    assert_ne!(
        base.generation_inputs_signature(),
        retooled.generation_inputs_signature()
    );
}
