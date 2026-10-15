//! W2.4 — every op states its capabilities, and the answers did not move.
//!
//! # What changed
//!
//! The capability answers were `match` lists outside the registry:
//! `transform_capabilities_without_lap_cap`, `air_cut_high_threshold_pct`,
//! `honors_pinned_bottom_z` and `entry_probe_leave` in `catalog.rs`,
//! `link_ceiling_need` in `execute/shared.rs`, two lists in
//! `feeds/predict.rs` (one ended `_ => {}`, so a new op failed open), one
//! in `feeds/suggest/axial_envelope.rs` and three in `trace/narrate.rs`.
//! Each registry row now states one `Capabilities` value, and
//! `OperationConfig::capabilities` overrides the fields that read the
//! config.
//!
//! # The proof
//!
//! Before the old functions were deleted, a temporary test asserted that
//! `capabilities()` equals each old function for every op (and for each
//! surface op with a non-default leave, and for Scallop both ways), and
//! that the narration classes equal the three old narrate lists. That run
//! was green. The table below is the frozen output of that run. It is the
//! expected value from now on: a change to a row is a change of behaviour,
//! and it must change this table on purpose.
//!
//! W3.4 added the `rest_use` column: the rest rule of each op (AUDIT.md,
//! "Rest is not one thing"), read by the Layered runner and the drill.
//! W3.5 moved Chamfer onto the Layered runner: its row reads `LevelSkip`.
//! W6.2 added the `territory_xy` column: the XY containment each op reads
//! while it plans, resolved once per op by `compute::execute::Territory`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{
    AirCutHint, AxialEnvelope, Capabilities, DocBasis, EntryProbe, EntryProbeRule, MoveCountModel,
    OperationConfig, OperationType, RestUse, RowCapabilities, TerritoryXy,
};
use rs_cam_core::compute::execute::LinkCeilingNeed;

/// One frozen row.
struct Row {
    op: OperationType,
    /// `allows_rapid_reorder`, `allows_global_rapid_reorder`,
    /// `requires_depth_order`, `continuous_path_required`,
    /// `allows_link_moves`, `planner_applies_segment_merge`,
    /// `helix_floor_lap`, `stock_aware_plunge`.
    transform: [bool; 8],
    lap_cap: Option<u32>,
    air_cut_pct: Option<f64>,
    honors_pinned_bottom_z: bool,
    entry_probe: EntryProbeRule,
    link_ceiling: LinkCeilingNeed,
    move_count: MoveCountModel,
    doc_basis: DocBasis,
    air_cut_hint: AirCutHint,
    relink_dial: Option<&'static str>,
    axial_envelope: AxialEnvelope,
    /// W3.4: how the op uses the remaining stock.
    rest_use: RestUse,
    /// W6.2: the XY containment the op reads while it plans.
    territory_xy: TerritoryXy,
}

/// The columns of one frozen row, in [`Row`] field order.
type Columns = (
    OperationType,
    [bool; 8],
    Option<u32>,
    Option<f64>,
    bool,
    EntryProbeRule,
    LinkCeilingNeed,
    MoveCountModel,
    DocBasis,
    AirCutHint,
    Option<&'static str>,
    AxialEnvelope,
    RestUse,
    TerritoryXy,
);

fn row(c: Columns) -> Row {
    Row {
        op: c.0,
        transform: c.1,
        lap_cap: c.2,
        air_cut_pct: c.3,
        honors_pinned_bottom_z: c.4,
        entry_probe: c.5,
        link_ceiling: c.6,
        move_count: c.7,
        doc_basis: c.8,
        air_cut_hint: c.9,
        relink_dial: c.10,
        axial_envelope: c.11,
        rest_use: c.12,
        territory_xy: c.13,
    }
}

#[rustfmt::skip]
fn frozen() -> Vec<Row> {
    vec![
    row((OperationType::Face, [false, false, false, false, false, false, false, false], None, Some(40.0), false, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::LevelClip, TerritoryXy::ClipAfter)),
    row((OperationType::Pocket, [true, false, true, false, false, false, false, false], None, Some(40.0), false, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::PocketCell, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::LevelClip, TerritoryXy::ClipAfter)),
    row((OperationType::Profile, [true, false, true, false, false, false, false, false], None, Some(40.0), false, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::LevelSkip, TerritoryXy::ClipAfter)),
    row((OperationType::Adaptive, [false, false, true, false, false, true, true, true], None, Some(40.0), false, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::Adaptive2d, DocBasis::DepthPerPass, AirCutHint::Roughing, None, AxialEnvelope::NotApplied, RestUse::LevelMask, TerritoryXy::ClipAfter)),
    row((OperationType::VCarve, [true, false, false, false, false, false, false, false], Some(3), Some(40.0), false, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::VBitDepth, AirCutHint::CurveFollowing, None, AxialEnvelope::VBitGroove, RestUse::None, TerritoryXy::ClipAfter)),
    row((OperationType::Inlay, [true, false, false, false, false, false, false, false], Some(3), Some(40.0), false, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::None, TerritoryXy::ClipAfter)),
    row((OperationType::InlayPlug, [true, false, false, false, false, false, false, false], Some(3), Some(40.0), false, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::None, TerritoryXy::ClipAfter)),
    row((OperationType::Zigzag, [true, false, true, false, false, false, false, false], None, Some(40.0), false, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::Raster, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::LevelClip, TerritoryXy::ClipAfter)),
    row((OperationType::Trace, [true, false, true, false, false, false, false, false], Some(3), Some(40.0), false, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::CurveFollowing, None, AxialEnvelope::NotApplied, RestUse::LevelSkip, TerritoryXy::ClipAfter)),
    row((OperationType::Drill, [true, true, false, false, true, false, false, false], None, None, false, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::PeckDepth, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::StockColumn, TerritoryXy::ClipAfter)),
    row((OperationType::Chamfer, [true, false, false, false, true, false, false, false], Some(3), Some(40.0), false, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::LevelSkip, TerritoryXy::ClipAfter)),
    row((OperationType::DropCutter, [true, true, false, false, true, false, false, false], Some(3), Some(45.0), false, EntryProbeRule::SurfaceFixed { leave_mm: 0.0 }, LinkCeilingNeed::WithRemainingStock, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, Some("hookup_mm"), AxialEnvelope::Finish3d, RestUse::LinkSafetyOnly, TerritoryXy::Regions)),
    row((OperationType::Adaptive3d, [false, false, true, false, false, true, false, false], None, Some(40.0), true, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::Adaptive3d, DocBasis::DepthPerPass, AirCutHint::Roughing, None, AxialEnvelope::Roughing3d, RestUse::Planner, TerritoryXy::PlannerBounds)),
    row((OperationType::Waterline, [true, false, true, false, false, false, false, false], Some(3), Some(45.0), true, EntryProbeRule::SurfaceFixed { leave_mm: 0.0 }, LinkCeilingNeed::WithRemainingStock, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, Some("hookup_mm"), AxialEnvelope::Finish3d, RestUse::ContourFilter, TerritoryXy::Regions)),
    row((OperationType::Pencil, [true, false, false, false, true, false, false, false], Some(3), Some(45.0), false, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::WithRemainingStock, MoveCountModel::FeatureDriven, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, None, AxialEnvelope::NotApplied, RestUse::Reference, TerritoryXy::ClipAfter)),
    row((OperationType::Scallop, [true, false, false, true, false, false, false, false], Some(3), Some(45.0), false, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::WithRemainingStock, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, Some("intra_pass_hookup_mm"), AxialEnvelope::Finish3d, RestUse::LinkSafetyOnly, TerritoryXy::Regions)),
    row((OperationType::UnifiedFinish, [true, false, false, false, false, false, false, false], Some(3), Some(45.0), true, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::WithRemainingStock, MoveCountModel::NotModelled, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, Some("intra_region_hookup_mm"), AxialEnvelope::Finish3d, RestUse::Reference, TerritoryXy::Regions)),
    row((OperationType::SteepShallow, [true, false, false, false, false, false, false, false], Some(3), Some(45.0), false, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::NoLink, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, None, AxialEnvelope::Finish3d, RestUse::None, TerritoryXy::Regions)),
    row((OperationType::RampFinish, [true, false, false, true, false, false, false, false], Some(3), Some(45.0), false, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, None, AxialEnvelope::NotApplied, RestUse::None, TerritoryXy::Regions)),
    row((OperationType::SpiralFinish, [true, false, false, true, false, false, false, false], Some(3), Some(45.0), false, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::NoLink, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, None, AxialEnvelope::Finish3d, RestUse::None, TerritoryXy::Regions)),
    row((OperationType::RadialFinish, [true, false, false, false, true, false, false, false], Some(3), Some(45.0), false, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::NoLink, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, None, AxialEnvelope::Finish3d, RestUse::None, TerritoryXy::Regions)),
    row((OperationType::HorizontalFinish, [true, false, false, false, true, false, false, false], Some(3), Some(45.0), false, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::NoLink, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, None, AxialEnvelope::Finish3d, RestUse::None, TerritoryXy::Regions)),
    row((OperationType::ProjectCurve, [true, true, false, false, true, false, false, false], Some(3), Some(60.0), false, EntryProbeRule::Prism, LinkCeilingNeed::Always, MoveCountModel::FeatureDriven, DocBasis::CurveOnSurface, AirCutHint::CurveFollowing, None, AxialEnvelope::CurveRough, RestUse::LinkSafetyOnly, TerritoryXy::Regions)),
    row((OperationType::AlignmentPinDrill, [true, true, false, false, true, false, false, false], None, None, false, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::PeckDepth, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::None, TerritoryXy::ClipAfter)),
    ]
}

fn transform_bits<P>(c: &Capabilities<P>) -> [bool; 8] {
    let t = c.transform;
    [
        t.allows_rapid_reorder,
        t.allows_global_rapid_reorder,
        t.requires_depth_order,
        t.continuous_path_required,
        t.allows_link_moves,
        t.planner_applies_segment_merge,
        t.helix_floor_lap,
        t.stock_aware_plunge,
    ]
}

fn assert_row(c: &RowCapabilities, r: &Row, what: &str) {
    let op = r.op;
    assert_eq!(transform_bits(c), r.transform, "{op:?} ({what}): transform");
    assert_eq!(
        c.transform.ramp_fold_lap_cap, r.lap_cap,
        "{op:?} ({what}): lap cap"
    );
    assert_eq!(
        c.air_cut_high_threshold_pct, r.air_cut_pct,
        "{op:?} ({what}): air-cut bar"
    );
    assert_eq!(
        c.honors_pinned_bottom_z, r.honors_pinned_bottom_z,
        "{op:?} ({what}): pinned bottom"
    );
    assert_eq!(c.entry_probe, r.entry_probe, "{op:?} ({what}): probe");
    assert_eq!(
        c.link_ceiling, r.link_ceiling,
        "{op:?} ({what}): link ceiling"
    );
    assert_eq!(c.move_count, r.move_count, "{op:?} ({what}): move count");
    assert_eq!(c.doc_basis, r.doc_basis, "{op:?} ({what}): DOC basis");
    assert_eq!(
        c.air_cut_hint, r.air_cut_hint,
        "{op:?} ({what}): air-cut hint"
    );
    assert_eq!(c.relink_dial, r.relink_dial, "{op:?} ({what}): relink dial");
    assert_eq!(
        c.axial_envelope, r.axial_envelope,
        "{op:?} ({what}): axial envelope"
    );
    assert_eq!(c.rest_use, r.rest_use, "{op:?} ({what}): rest use");
    assert_eq!(
        c.territory_xy, r.territory_xy,
        "{op:?} ({what}): territory xy"
    );
}

/// The table covers the registry, one row per op.
#[test]
fn the_frozen_table_covers_every_op() {
    let ops: Vec<OperationType> = frozen().iter().map(|r| r.op).collect();
    assert_eq!(ops, OperationType::ALL.to_vec());
}

/// The row of each op kind equals the frozen answer.
#[test]
fn every_op_kind_keeps_its_capabilities() {
    for r in frozen() {
        assert_row(&r.op.capabilities(), &r, "kind");
    }
}

/// A default config gives its kind's row, except for the fields that read
/// the config: the default discrete Scallop, the leave of each surface op
/// and the feeds hints.
#[test]
fn a_default_config_overrides_only_what_it_reads() {
    for r in frozen() {
        let cfg = OperationConfig::new_default(r.op);
        let c = cfg.capabilities();
        let kind = r.op.capabilities();
        assert_eq!(c.move_count, kind.move_count, "{:?}", r.op);
        assert_eq!(c.doc_basis, kind.doc_basis, "{:?}", r.op);
        assert_eq!(c.air_cut_hint, kind.air_cut_hint, "{:?}", r.op);
        assert_eq!(c.relink_dial, kind.relink_dial, "{:?}", r.op);
        assert_eq!(c.axial_envelope, kind.axial_envelope, "{:?}", r.op);
        assert_eq!(c.link_ceiling, kind.link_ceiling, "{:?}", r.op);
        assert_eq!(c.rest_use, kind.rest_use, "{:?}", r.op);
        assert_eq!(c.territory_xy, kind.territory_xy, "{:?}", r.op);
        assert_eq!(
            c.air_cut_high_threshold_pct,
            kind.air_cut_high_threshold_pct
        );
        assert_eq!(c.honors_pinned_bottom_z, kind.honors_pinned_bottom_z);
        if r.op != OperationType::Scallop {
            assert_eq!(c.transform, kind.transform, "{:?}", r.op);
        }
        // A prism op never gets a probe; a surface op always does, and a
        // fixed leave is the row's own number.
        let expected = match r.entry_probe {
            EntryProbeRule::Prism => EntryProbe::Prism,
            EntryProbeRule::SurfaceFixed { leave_mm } => EntryProbe::Surface { leave_mm },
            EntryProbeRule::SurfaceFromConfig => EntryProbe::Surface {
                leave_mm: cfg.stock_to_leave_dial().unwrap(),
            },
        };
        assert_eq!(c.entry_probe, expected, "{:?}", r.op);
    }
}

/// A row says `SurfaceFromConfig` exactly when the config has a
/// `stock_to_leave` dial, so the row states no placeholder leave and the
/// resolution never falls back.
#[test]
fn a_leave_rule_reads_from_config_only_where_a_dial_exists() {
    for r in frozen() {
        let dial = OperationConfig::new_default(r.op).stock_to_leave_dial();
        assert_eq!(
            r.entry_probe == EntryProbeRule::SurfaceFromConfig,
            dial.is_some(),
            "{:?}",
            r.op
        );
    }
}

/// Discrete-ring Scallop (the default) takes global reorder, no links and
/// NO lap cap (the per-instance override carried none before W2.4);
/// continuous Scallop keeps the kind's row.
#[test]
fn scallop_continuity_picks_its_transform() {
    let mut discrete = OperationConfig::new_default(OperationType::Scallop);
    let OperationConfig::Scallop(cfg) = &mut discrete else {
        unreachable!()
    };
    cfg.continuous = false;
    let c = discrete.capabilities();
    assert_eq!(
        transform_bits(&c),
        [true, true, false, false, false, false, false, false]
    );
    assert_eq!(c.transform.ramp_fold_lap_cap, None);

    let mut continuous = discrete.clone();
    let OperationConfig::Scallop(cfg) = &mut continuous else {
        unreachable!()
    };
    cfg.continuous = true;
    assert_eq!(
        continuous.capabilities().transform,
        OperationType::Scallop.capabilities().transform
    );
}

/// Each surface op with a `stock_to_leave` dial probes at that leave; the
/// two without one (Drop Cutter, Waterline) probe at the CL surface.
#[test]
fn a_surface_probe_reads_the_ops_own_leave() {
    let leave_ops = [
        OperationType::Pencil,
        OperationType::Scallop,
        OperationType::UnifiedFinish,
        OperationType::SteepShallow,
        OperationType::RampFinish,
        OperationType::SpiralFinish,
        OperationType::RadialFinish,
        OperationType::HorizontalFinish,
    ];
    for op in leave_ops {
        let mut value = serde_json::to_value(OperationConfig::new_default(op)).unwrap();
        value["params"]["stock_to_leave"] = serde_json::json!(0.437);
        let cfg: OperationConfig = serde_json::from_value(value).unwrap();
        assert_eq!(
            cfg.capabilities().entry_probe.leave_mm(),
            Some(0.437),
            "{op:?}"
        );
    }
    for op in [OperationType::DropCutter, OperationType::Waterline] {
        assert_eq!(
            OperationConfig::new_default(op)
                .capabilities()
                .entry_probe
                .leave_mm(),
            Some(0.0),
            "{op:?}"
        );
    }
}

/// The feeds hints read the op's own fields; every other op has none.
#[test]
fn the_feeds_hints_read_the_ops_own_fields() {
    let set = |op: OperationType, field: &str| {
        let mut value = serde_json::to_value(OperationConfig::new_default(op)).unwrap();
        value["params"][field] = serde_json::json!(0.437);
        let cfg: OperationConfig = serde_json::from_value(value).unwrap();
        cfg.capabilities().feeds_hints
    };
    for (op, field) in [
        (OperationType::Scallop, "scallop_height"),
        (OperationType::UnifiedFinish, "scallop_height"),
        (OperationType::DropCutter, "scallop_height"),
    ] {
        let h = set(op, field);
        assert_eq!(h.target_scallop_mm, Some(0.437), "{op:?}");
        assert_eq!(h.axial_depth_mm, None, "{op:?}");
    }
    for (op, field) in [
        (OperationType::Waterline, "z_step"),
        (OperationType::SteepShallow, "z_step"),
        (OperationType::VCarve, "max_depth"),
        (OperationType::RampFinish, "max_stepdown"),
    ] {
        let h = set(op, field);
        assert_eq!(h.axial_depth_mm, Some(0.437), "{op:?}");
        assert_eq!(h.target_scallop_mm, None, "{op:?}");
    }
    let hinted = [
        OperationType::Scallop,
        OperationType::UnifiedFinish,
        OperationType::DropCutter,
        OperationType::Waterline,
        OperationType::SteepShallow,
        OperationType::VCarve,
        OperationType::RampFinish,
    ];
    for &op in OperationType::ALL {
        if hinted.contains(&op) {
            continue;
        }
        let h = OperationConfig::new_default(op).capabilities().feeds_hints;
        assert_eq!(h.axial_depth_mm, None, "{op:?}");
        assert_eq!(h.radial_width_mm, None, "{op:?}");
        assert_eq!(h.target_scallop_mm, None, "{op:?}");
    }
}
