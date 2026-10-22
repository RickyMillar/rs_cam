//! W2.4 — every op states its capabilities, and the answers did not move.
//!
//! # What changed
//!
//! The capability answers were `match` lists outside the registry:
//! `transform_capabilities_without_lap_cap`, `air_cut_high_threshold_pct`,
//! `floor_owner` and `entry_probe_leave` in `catalog.rs`,
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
//!
//! C2 added eight columns in a second table ([`frozen_c2`]). Each one
//! replaced a `match` on the op type in a consumer:
//!
//! - `plans_bands`: the dropped-band and clipped-band narrate lines.
//! - `ramp_descent`: the ramp reach clamp narrate line.
//! - `adaptive_woc`: the 0.15 x D wood stepover floor in `validate.rs`.
//! - `flat_floor_clearing`: the ball-nose hint in the static checks.
//! - `clearing_strategy_dial`: the strategy advisor guard and its write.
//! - `planner_uniform_engagement`: the modulation planner floor. The row
//!   says `false`; a contour-spiral config says `true`.
//! - `top_on_stock_box`: the pinned-bottom check of 3D Rough.
//! - `floor_inside_stock`: the depth-beyond-stock check.
//!
//! Before C2 changed each consumer, the old lists gave the same sets.
//!
//! D2 changed three answers on purpose:
//!
//! - `lut_route` (a third table, [`frozen_d2`]): the vendor-table routing
//!   of the feeds lookup. It replaced three op-type compares in
//!   `feeds::vendor_normalize::lut_query_for` and one in
//!   `feeds::explain`, with the same routes: 3D Rough `AdaptiveAsPocket`,
//!   Drop Cutter `FlatParallelAsRoughPocket`, Project Curve `ByCutter`,
//!   every other op `AsStated`.
//! - Project Curve `relink_dial`: `None` -> `chain_distance_mm`. Its runner
//!   now records the chain-link totals, and the narration names this dial
//!   as their lever.
//! - Pin Drill `territory_xy`: `ClipAfter` -> `SetupMap` (lead ruling: the
//!   pin holes are absolute setup-map positions through the whole stock,
//!   so no territory clip applies; the obstacle step still does).
//!
//! P1 (geometry selection, 2026-10-19) added the `item_use` column (a
//! fourth table, [`frozen_p1`]): how the op cuts its drawing items. The
//! prologue gives a `Regions` op the closed polygons only and a `Curves`
//! op every polygon. Trace read its compensation: `None` was `Curves`,
//! `Left` and `Right` were `Regions`. The column is new; no earlier answer
//! moved. P6.2 (the open offset): Trace is `Curves` for every compensation
//! ([`trace_compensation_picks_its_item_use_p1`] moved by name). P6.3:
//! Profile side `On` is `Curves` ([`profile_side_picks_its_item_use_p6_3`]),
//! and the Profile transform moved by name to `TRANSFORM_PROFILE` (L-2):
//! `leads_on_closed_passes_only` is `true`, so an open pass of side `On`
//! takes no lead. A closed pass keeps its lead. [`every_field_has_a_frozen_column`] destructures `Capabilities`
//! with no `..`, so the next new field does not compile here until it has
//! a column.
//!
//! F033b (lead ruling, 2026-10-21) added the `ladder_includes_top` column
//! (a fifth table, [`frozen_f033b`]): the op's depth ladder includes its
//! top level (`LadderMode::ConstantStep`). `height_order` then refuses
//! only a bottom above the top. Waterline, Steep and Shallow and Ramp
//! Finish state `true`. The column is new; no earlier answer moved. Unified
//! Finish states `true` too: its VerySteep band ladders from the Top Z with
//! `waterline_z_levels` (`ConstantStep`).
//!
//! P6.0b (geometry selection, SAFETY) added the tenth transform bit,
//! `finished_edge`: the op's cut can be a finished edge, so the dressup
//! segment merge runs at most at `FlattenPolicy::UNTOLERANCED_MM`. Face
//! (no wall) and 3D Rough (a rough with a leave; its planner merges)
//! state `false`; every other op states `true`. The bit is new; no
//! earlier answer moved.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{
    AirCutHint, AxialEnvelope, Capabilities, DocBasis, EntryProbe, EntryProbeRule, FloorOwner,
    GeometryRequirement, ItemUse, LutRoute, MoveCountModel, OperationConfig, OperationType,
    RestUse, RowCapabilities, TerritoryXy,
};
use rs_cam_core::compute::execute::LinkCeilingNeed;
use rs_cam_core::ops::trace_path::TraceCompensation;
use rs_cam_core::selection::HoleFitRule;

/// One frozen row.
struct Row {
    op: OperationType,
    /// `allows_rapid_reorder`, `allows_global_rapid_reorder`,
    /// `requires_depth_order`, `continuous_path_required`,
    /// `allows_link_moves`, `planner_applies_segment_merge`,
    /// `helix_floor_lap`, `stock_aware_plunge`,
    /// `leads_on_closed_passes_only` (P1b), `finished_edge` (P6.0b).
    transform: [bool; 10],
    lap_cap: Option<u32>,
    air_cut_pct: Option<f64>,
    floor_owner: FloorOwner,
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
    [bool; 10],
    Option<u32>,
    Option<f64>,
    FloorOwner,
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
        floor_owner: c.4,
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
    row((OperationType::Face, [false, false, false, false, false, false, false, false, false, false], None, Some(40.0), FloorOwner::OpDial, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::LevelClip, TerritoryXy::ClipAfter)),
    row((OperationType::Pocket, [true, false, true, false, false, false, false, false, false, true], None, Some(40.0), FloorOwner::OpFloor, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::PocketCell, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::LevelClip, TerritoryXy::ClipAfter)),
    row((OperationType::Profile, [true, false, true, false, false, false, false, false, true, true], None, Some(40.0), FloorOwner::OpFloor, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::LevelSkip, TerritoryXy::ClipAfter)),
    row((OperationType::Adaptive, [false, false, true, false, false, true, true, true, false, true], None, Some(40.0), FloorOwner::OpFloor, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::Adaptive2d, DocBasis::DepthPerPass, AirCutHint::Roughing, None, AxialEnvelope::NotApplied, RestUse::LevelMask, TerritoryXy::ClipAfter)),
    row((OperationType::VCarve, [true, false, false, false, false, false, false, false, false, true], Some(3), Some(40.0), FloorOwner::OpDial, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::VBitDepth, AirCutHint::CurveFollowing, None, AxialEnvelope::VBitGroove, RestUse::None, TerritoryXy::ClipAfter)),
    row((OperationType::Inlay, [true, false, false, false, false, false, false, false, false, true], Some(3), Some(40.0), FloorOwner::OpDial, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::None, TerritoryXy::ClipAfter)),
    row((OperationType::InlayPlug, [true, false, false, false, false, false, false, false, false, true], Some(3), Some(40.0), FloorOwner::OpDial, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::None, TerritoryXy::ClipAfter)),
    row((OperationType::Zigzag, [true, false, true, false, false, false, false, false, false, true], None, Some(40.0), FloorOwner::OpFloor, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::Raster, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::LevelClip, TerritoryXy::ClipAfter)),
    row((OperationType::Trace, [true, false, true, false, false, false, false, false, true, true], Some(3), Some(40.0), FloorOwner::OpDial, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::CurveFollowing, None, AxialEnvelope::NotApplied, RestUse::LevelSkip, TerritoryXy::ClipAfter)),
    row((OperationType::Drill, [true, true, false, false, true, false, false, false, false, true], None, None, FloorOwner::OpDial, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::PeckDepth, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::StockColumn, TerritoryXy::ClipAfter)),
    row((OperationType::Bore, [true, true, false, false, true, false, false, false, false, true], None, Some(40.0), FloorOwner::OpFloor, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::StockColumn, TerritoryXy::ClipAfter)),
    row((OperationType::Chamfer, [true, false, false, false, true, false, false, false, false, true], Some(3), Some(40.0), FloorOwner::OpDial, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::DepthPerPass, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::LevelSkip, TerritoryXy::ClipAfter)),
    row((OperationType::DropCutter, [true, true, false, false, true, false, false, false, false, true], Some(3), Some(45.0), FloorOwner::OpDial, EntryProbeRule::SurfaceFixed { leave_mm: 0.0 }, LinkCeilingNeed::WithRemainingStock, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, Some("hookup_mm"), AxialEnvelope::Finish3d, RestUse::LinkSafetyOnly, TerritoryXy::Regions)),
    row((OperationType::Adaptive3d, [false, false, true, false, false, true, false, false, false, false], None, Some(40.0), FloorOwner::HeightsBottom, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::Adaptive3d, DocBasis::DepthPerPass, AirCutHint::Roughing, None, AxialEnvelope::Roughing3d, RestUse::Planner, TerritoryXy::PlannerBounds)),
    row((OperationType::Waterline, [true, false, true, false, false, false, false, false, false, true], Some(3), Some(45.0), FloorOwner::HeightsBottom, EntryProbeRule::SurfaceFixed { leave_mm: 0.0 }, LinkCeilingNeed::WithRemainingStock, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, Some("hookup_mm"), AxialEnvelope::Finish3d, RestUse::ContourFilter, TerritoryXy::Regions)),
    row((OperationType::Pencil, [true, false, false, false, true, false, false, false, false, true], Some(3), Some(45.0), FloorOwner::OpDial, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::WithRemainingStock, MoveCountModel::FeatureDriven, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, None, AxialEnvelope::NotApplied, RestUse::Reference, TerritoryXy::ClipAfter)),
    row((OperationType::Scallop, [true, false, false, true, false, false, false, false, false, true], Some(3), Some(45.0), FloorOwner::OpDial, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::WithRemainingStock, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, Some("intra_pass_hookup_mm"), AxialEnvelope::Finish3d, RestUse::LinkSafetyOnly, TerritoryXy::Regions)),
    row((OperationType::UnifiedFinish, [true, false, false, false, false, false, false, false, false, true], Some(3), Some(45.0), FloorOwner::HeightsBottom, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::WithRemainingStock, MoveCountModel::NotModelled, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, Some("intra_region_hookup_mm"), AxialEnvelope::Finish3d, RestUse::Reference, TerritoryXy::Regions)),
    row((OperationType::SteepShallow, [true, false, false, false, false, false, false, false, false, true], Some(3), Some(45.0), FloorOwner::OpDial, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::WithRemainingStock, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, Some("hookup_mm"), AxialEnvelope::Finish3d, RestUse::ContourFilter, TerritoryXy::Regions)),
    row((OperationType::RampFinish, [true, false, false, true, false, false, false, false, false, true], Some(3), Some(45.0), FloorOwner::OpDial, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, None, AxialEnvelope::NotApplied, RestUse::None, TerritoryXy::Regions)),
    row((OperationType::SpiralFinish, [true, false, false, true, false, false, false, false, false, true], Some(3), Some(45.0), FloorOwner::OpDial, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::NoLink, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, None, AxialEnvelope::Finish3d, RestUse::None, TerritoryXy::Regions)),
    row((OperationType::RadialFinish, [true, false, false, false, true, false, false, false, false, true], Some(3), Some(45.0), FloorOwner::OpDial, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::NoLink, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, None, AxialEnvelope::Finish3d, RestUse::None, TerritoryXy::Regions)),
    row((OperationType::HorizontalFinish, [true, false, false, false, true, false, false, false, false, true], Some(3), Some(45.0), FloorOwner::OpDial, EntryProbeRule::SurfaceFromConfig, LinkCeilingNeed::NoLink, MoveCountModel::Raster, DocBasis::SurfaceHeights, AirCutHint::SurfaceFinish, None, AxialEnvelope::Finish3d, RestUse::None, TerritoryXy::Regions)),
    row((OperationType::ProjectCurve, [true, true, false, false, true, false, false, false, false, true], Some(3), Some(60.0), FloorOwner::OpDial, EntryProbeRule::Prism, LinkCeilingNeed::Always, MoveCountModel::FeatureDriven, DocBasis::CurveOnSurface, AirCutHint::CurveFollowing, Some("chain_distance_mm"), AxialEnvelope::CurveRough, RestUse::LinkSafetyOnly, TerritoryXy::Regions)),
    row((OperationType::AlignmentPinDrill, [true, true, false, false, true, false, false, false, false, true], None, None, FloorOwner::OpDial, EntryProbeRule::Prism, LinkCeilingNeed::NoLink, MoveCountModel::FeatureDriven, DocBasis::PeckDepth, AirCutHint::NoHint, None, AxialEnvelope::NotApplied, RestUse::None, TerritoryXy::SetupMap)),
    ]
}

fn transform_bits<P>(c: &Capabilities<P>) -> [bool; 10] {
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
        t.leads_on_closed_passes_only,
        t.finished_edge,
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
    assert_eq!(c.floor_owner, r.floor_owner, "{op:?} ({what}): floor owner");
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
        assert_eq!(c.floor_owner, kind.floor_owner);
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
        [
            true, true, false, false, false, false, false, false, false, true
        ]
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

/// One frozen C2 row: the eight columns C2 added, in field order.
type C2Row = (
    OperationType,
    bool,
    bool,
    bool,
    bool,
    Option<&'static str>,
    bool,
    bool,
    bool,
);

/// The C2 columns: `plans_bands`, `ramp_descent`, `adaptive_woc`,
/// `flat_floor_clearing`, `clearing_strategy_dial`,
/// `planner_uniform_engagement`, `top_on_stock_box`, `floor_inside_stock`.
#[rustfmt::skip]
fn frozen_c2() -> Vec<C2Row> {
    use OperationType as O;
    vec![
    (O::Face, false, false, false, true, None, false, false, true),
    (O::Pocket, false, false, false, true, None, false, false, true),
    (O::Profile, false, false, false, false, None, false, false, true),
    (O::Adaptive, false, false, true, false, None, false, false, true),
    (O::VCarve, false, false, false, false, None, false, false, true),
    (O::Inlay, false, false, false, false, None, false, false, true),
    (O::InlayPlug, false, false, false, false, None, false, false, true),
    (O::Zigzag, false, false, false, true, None, false, false, true),
    (O::Trace, false, false, false, false, None, false, false, true),
    (O::Drill, false, false, false, false, None, false, false, true),
    (O::Bore, false, false, false, true, None, false, false, true),
    (O::Chamfer, false, false, false, false, None, false, false, true),
    (O::DropCutter, false, false, false, false, None, false, false, true),
    (O::Adaptive3d, false, false, true, false, Some("clearing_strategy"), false, true, true),
    (O::Waterline, false, false, false, false, None, false, false, true),
    (O::Pencil, false, false, false, false, None, false, false, true),
    (O::Scallop, false, false, false, false, None, false, false, true),
    (O::UnifiedFinish, true, false, false, false, None, false, false, true),
    (O::SteepShallow, false, false, false, false, None, false, false, true),
    (O::RampFinish, false, true, false, false, None, false, false, true),
    (O::SpiralFinish, false, false, false, false, None, false, false, true),
    (O::RadialFinish, false, false, false, false, None, false, false, true),
    (O::HorizontalFinish, false, false, false, false, None, false, false, true),
    (O::ProjectCurve, false, false, false, false, None, false, false, false),
    (O::AlignmentPinDrill, false, false, false, false, None, false, false, false),
    ]
}

fn c2_columns<P>(
    c: &Capabilities<P>,
) -> (
    bool,
    bool,
    bool,
    bool,
    Option<&'static str>,
    bool,
    bool,
    bool,
) {
    (
        c.plans_bands,
        c.ramp_descent,
        c.adaptive_woc,
        c.flat_floor_clearing,
        c.clearing_strategy_dial,
        c.planner_uniform_engagement,
        c.top_on_stock_box,
        c.floor_inside_stock,
    )
}

/// C2: the row of each op kind and its default config give the frozen
/// C2 columns. No default config uses the contour-spiral strategy, so
/// no default config overrides a C2 column.
#[test]
fn every_op_keeps_its_c2_capabilities() {
    let table = frozen_c2();
    let ops: Vec<OperationType> = table.iter().map(|r| r.0).collect();
    assert_eq!(ops, OperationType::ALL.to_vec());
    for r in table {
        let expected = (r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8);
        assert_eq!(
            c2_columns(&r.0.capabilities()),
            expected,
            "{:?} (kind)",
            r.0
        );
        let cfg = OperationConfig::new_default(r.0);
        assert_eq!(
            c2_columns(&cfg.capabilities()),
            expected,
            "{:?} (config)",
            r.0
        );
    }
}

/// C2: a contour-spiral strategy, 2D or 3D, states the uniform planner
/// engagement; every other strategy does not.
#[test]
fn only_a_contour_spiral_holds_a_uniform_engagement() {
    let with = |op: OperationType, field: &str, token: &str| {
        let mut value = serde_json::to_value(OperationConfig::new_default(op)).unwrap();
        value["params"][field] = serde_json::json!(token);
        let cfg: OperationConfig = serde_json::from_value(value).unwrap();
        cfg.capabilities().planner_uniform_engagement
    };
    assert!(with(
        OperationType::Adaptive3d,
        "clearing_strategy",
        "contour_spiral"
    ));
    for token in ["contour_parallel", "adaptive", "agent_search"] {
        assert!(
            !with(OperationType::Adaptive3d, "clearing_strategy", token),
            "{token}"
        );
    }
    assert!(with(
        OperationType::Adaptive,
        "path_strategy",
        "ContourSpiral"
    ));
    assert!(!with(OperationType::Adaptive, "path_strategy", "Agent"));
}

/// C2: a dial that a capability names by its config field name is a
/// schema parameter of that op. The advisor writes `clearing_strategy_dial`
/// by name through a serde round trip, and a renamed field would make that
/// write fail with no message.
#[test]
fn every_named_dial_is_a_schema_parameter() {
    let mut named = 0;
    for &op in OperationType::ALL {
        let caps = op.capabilities();
        for dial in [caps.clearing_strategy_dial, caps.relink_dial]
            .into_iter()
            .flatten()
        {
            named += 1;
            assert!(
                op.registry_entry()
                    .params
                    .defs
                    .iter()
                    .any(|d| d.name == dial),
                "{op:?}: the dial `{dial}` is not a schema parameter"
            );
        }
    }
    assert!(named >= 5, "non-vacuity: {named} named dials");
}

/// D2: the `lut_route` column, frozen per op kind.
#[rustfmt::skip]
fn frozen_d2() -> Vec<(OperationType, LutRoute)> {
    use OperationType as O;
    vec![
    (O::Face, LutRoute::AsStated),
    (O::Pocket, LutRoute::AsStated),
    (O::Profile, LutRoute::AsStated),
    (O::Adaptive, LutRoute::AsStated),
    (O::VCarve, LutRoute::AsStated),
    (O::Inlay, LutRoute::AsStated),
    (O::InlayPlug, LutRoute::AsStated),
    (O::Zigzag, LutRoute::AsStated),
    (O::Trace, LutRoute::AsStated),
    (O::Drill, LutRoute::AsStated),
    (O::Bore, LutRoute::AsStated),
    (O::Chamfer, LutRoute::AsStated),
    (O::DropCutter, LutRoute::FlatParallelAsRoughPocket),
    (O::Adaptive3d, LutRoute::AdaptiveAsPocket),
    (O::Waterline, LutRoute::AsStated),
    (O::Pencil, LutRoute::AsStated),
    (O::Scallop, LutRoute::AsStated),
    (O::UnifiedFinish, LutRoute::AsStated),
    (O::SteepShallow, LutRoute::AsStated),
    (O::RampFinish, LutRoute::AsStated),
    (O::SpiralFinish, LutRoute::AsStated),
    (O::RadialFinish, LutRoute::AsStated),
    (O::HorizontalFinish, LutRoute::AsStated),
    (O::ProjectCurve, LutRoute::ByCutter),
    (O::AlignmentPinDrill, LutRoute::AsStated),
    ]
}

/// D2: the row of each op kind and its default config give the frozen
/// `lut_route`. No config overrides it.
#[test]
fn every_op_keeps_its_lut_route_d2() {
    let table = frozen_d2();
    let ops: Vec<OperationType> = table.iter().map(|r| r.0).collect();
    assert_eq!(ops, OperationType::ALL.to_vec());
    for (op, route) in table {
        assert_eq!(op.capabilities().lut_route, route, "{op:?} (kind)");
        assert_eq!(
            OperationConfig::new_default(op).capabilities().lut_route,
            route,
            "{op:?} (config)"
        );
    }
}

/// D2: only the Pin Drill escapes the session boundary clip, and it is
/// the only `SetupMap` row.
#[test]
fn only_the_pin_drill_escapes_the_boundary_clip_d2() {
    for &op in OperationType::ALL {
        let xy = op.capabilities().territory_xy;
        assert_eq!(
            xy == TerritoryXy::SetupMap,
            op == OperationType::AlignmentPinDrill,
            "{op:?}: {xy:?}"
        );
        assert_eq!(xy.session_clip_applies(), xy != TerritoryXy::SetupMap);
    }
}

/// P1: the `item_use` column, frozen per op kind.
#[rustfmt::skip]
fn frozen_p1() -> Vec<(OperationType, ItemUse)> {
    use OperationType as O;
    vec![
    (O::Face, ItemUse::Regions),
    (O::Pocket, ItemUse::Regions),
    (O::Profile, ItemUse::Regions),
    (O::Adaptive, ItemUse::Regions),
    (O::VCarve, ItemUse::Regions),
    (O::Inlay, ItemUse::Regions),
    (O::InlayPlug, ItemUse::Regions),
    (O::Zigzag, ItemUse::Regions),
    (O::Trace, ItemUse::Curves),
    (O::Drill, ItemUse::Holes { may_be_empty: false, fit: HoleFitRule::ToolSize }),
    (O::Bore, ItemUse::Holes { may_be_empty: false, fit: HoleFitRule::LargerThanTool }),
    (O::Chamfer, ItemUse::Regions),
    (O::DropCutter, ItemUse::None),
    (O::Adaptive3d, ItemUse::None),
    (O::Waterline, ItemUse::None),
    (O::Pencil, ItemUse::None),
    (O::Scallop, ItemUse::None),
    (O::UnifiedFinish, ItemUse::None),
    (O::SteepShallow, ItemUse::None),
    (O::RampFinish, ItemUse::None),
    (O::SpiralFinish, ItemUse::None),
    (O::RadialFinish, ItemUse::None),
    (O::HorizontalFinish, ItemUse::None),
    (O::ProjectCurve, ItemUse::Curves),
    (O::AlignmentPinDrill, ItemUse::Holes { may_be_empty: true, fit: HoleFitRule::None }),
    ]
}

/// P1: the row of each op kind and its default config give the frozen
/// `item_use`. The default Trace (compensation `None`) keeps its row.
#[test]
fn every_op_keeps_its_item_use_p1() {
    let table = frozen_p1();
    let ops: Vec<OperationType> = table.iter().map(|r| r.0).collect();
    assert_eq!(ops, OperationType::ALL.to_vec());
    for (op, item_use) in table {
        assert_eq!(op.capabilities().item_use, item_use, "{op:?} (kind)");
        assert_eq!(
            OperationConfig::new_default(op).capabilities().item_use,
            item_use,
            "{op:?} (config)"
        );
    }
}

/// P1: Trace on the line follows any path. P6.2: Trace offset to one side
/// follows any path too (the open offset kernel), so every compensation
/// reads `Curves`. Before P6.2, `Left` and `Right` read `Regions`.
#[test]
fn trace_compensation_picks_its_item_use_p1() {
    for (compensation, expected) in [
        (TraceCompensation::None, ItemUse::Curves),
        (TraceCompensation::Left, ItemUse::Curves),
        (TraceCompensation::Right, ItemUse::Curves),
    ] {
        let mut cfg = OperationConfig::new_default(OperationType::Trace);
        let OperationConfig::Trace(trace) = &mut cfg else {
            unreachable!()
        };
        trace.compensation = compensation;
        assert_eq!(cfg.capabilities().item_use, expected, "{compensation:?}");
    }
}

/// P6.3: Profile side `On` cuts on the line, so it reads the lines of the
/// selection (`Curves`: closed rings and open paths). Outside and Inside
/// keep the row's closed areas (`Regions`).
#[test]
fn profile_side_picks_its_item_use_p6_3() {
    use rs_cam_core::ops::profile::ProfileSide;
    for (side, expected) in [
        (ProfileSide::Outside, ItemUse::Regions),
        (ProfileSide::Inside, ItemUse::Regions),
        (ProfileSide::On, ItemUse::Curves),
    ] {
        let mut cfg = OperationConfig::new_default(OperationType::Profile);
        let OperationConfig::Profile(profile) = &mut cfg else {
            unreachable!()
        };
        profile.side = side;
        assert_eq!(cfg.capabilities().item_use, expected, "{side:?}");
    }
}

/// P1, P2.3a (DESIGN 4.2): the geometry a row requires and the items it
/// cuts agree. A row reads items (`Items`, `ItemsAndMesh`) if and only if
/// its item use is not `None`. The prologue runs the selection view only
/// for those rows. A mesh op reads no item. Only Project Curve reads items
/// and a mesh, and it follows lines.
///
/// P2.3a moved Drill (was `DrillTargets`) and Pin Drill (was `Stock`) to
/// `Items`. Pin Drill may have no item (`Holes { may_be_empty: true }`):
/// its holes come from the stock pins, so it runs with no model
/// (`OperationType::is_stock_based`). P4.2 moved Face to `Items` with
/// `Regions`: its default selection is the stock outline, so it also runs
/// with no model.
#[test]
fn the_geometry_and_the_item_use_agree_p1() {
    for &op in OperationType::ALL {
        let use_ = op.capabilities().item_use;
        let ok = match op.geometry_requirement() {
            GeometryRequirement::Items => use_ != ItemUse::None,
            GeometryRequirement::ItemsAndMesh => use_ == ItemUse::Curves,
            GeometryRequirement::Mesh => use_ == ItemUse::None,
        };
        assert!(ok, "{op:?}: {:?} with {use_:?}", op.geometry_requirement());
    }
}

/// P2.3a: the frozen geometry column. Every row that read `Polygons` or
/// `DrillTargets` reads `Items`, Project Curve (was `Both`) reads
/// `ItemsAndMesh`, and Pin Drill moved from `Stock` to `Items`. P4.2 moved
/// Face to `Items` with the stock rule as its default (DESIGN D5), and
/// removed the `Stock` requirement. The last column is `is_stock_based`:
/// the ops that run with no model.
#[test]
fn every_row_states_its_geometry_p2_3a() {
    use GeometryRequirement::{Items, ItemsAndMesh, Mesh};
    let frozen: &[(&str, GeometryRequirement, bool)] = &[
        ("face", Items, true),
        ("pocket", Items, false),
        ("profile", Items, false),
        ("adaptive", Items, false),
        ("v_carve", Items, false),
        ("inlay", Items, false),
        ("inlay_plug", Items, false),
        ("zigzag", Items, false),
        ("trace", Items, false),
        ("drill", Items, false),
        ("bore", Items, false),
        ("chamfer", Items, false),
        ("drop_cutter", Mesh, false),
        ("adaptive3d", Mesh, false),
        ("waterline", Mesh, false),
        ("pencil", Mesh, false),
        ("scallop", Mesh, false),
        ("steep_shallow", Mesh, false),
        ("ramp_finish", Mesh, false),
        ("spiral_finish", Mesh, false),
        ("radial_finish", Mesh, false),
        ("horizontal_finish", Mesh, false),
        ("unified_finish", Mesh, false),
        ("project_curve", ItemsAndMesh, false),
        ("alignment_pin_drill", Items, true),
    ];
    let mut got: Vec<(&str, GeometryRequirement, bool)> = OperationType::ALL
        .iter()
        .map(|op| {
            (
                op.kind_str(),
                op.geometry_requirement(),
                op.is_stock_based(),
            )
        })
        .collect();
    let mut want = frozen.to_vec();
    got.sort_by_key(|row| row.0);
    want.sort_by_key(|row| row.0);
    assert_eq!(got, want, "the geometry column moved");
}

/// F033b: the `ladder_includes_top` column, frozen per op kind.
#[rustfmt::skip]
fn frozen_f033b() -> Vec<(OperationType, bool)> {
    use OperationType as O;
    vec![
    (O::Face, false),
    (O::Pocket, false),
    (O::Profile, false),
    (O::Adaptive, false),
    (O::VCarve, false),
    (O::Inlay, false),
    (O::InlayPlug, false),
    (O::Zigzag, false),
    (O::Trace, false),
    (O::Drill, false),
    (O::Bore, false),
    (O::Chamfer, false),
    (O::DropCutter, false),
    (O::Adaptive3d, false),
    (O::Waterline, true),
    (O::Pencil, false),
    (O::Scallop, false),
    (O::UnifiedFinish, true),
    (O::SteepShallow, true),
    (O::RampFinish, true),
    (O::SpiralFinish, false),
    (O::RadialFinish, false),
    (O::HorizontalFinish, false),
    (O::ProjectCurve, false),
    (O::AlignmentPinDrill, false),
    ]
}

/// F033b: the row of each op kind and its default config give the frozen
/// `ladder_includes_top`. No config overrides it.
#[test]
fn every_op_keeps_its_ladder_top_f033b() {
    let table = frozen_f033b();
    let ops: Vec<OperationType> = table.iter().map(|r| r.0).collect();
    assert_eq!(ops, OperationType::ALL.to_vec());
    for (op, includes_top) in table {
        assert_eq!(
            op.capabilities().ladder_includes_top,
            includes_top,
            "{op:?} (kind)"
        );
        assert_eq!(
            OperationConfig::new_default(op)
                .capabilities()
                .ladder_includes_top,
            includes_top,
            "{op:?} (config)"
        );
    }
}

/// P1: every `Capabilities` field has a frozen column in one of the five
/// tables, or a named test of its own. The destructure has no `..`, so a
/// new field does not compile here until this list (and a table) states it.
#[test]
fn every_field_has_a_frozen_column() {
    let Capabilities {
        transform: _,                  // frozen(), transform and lap cap
        air_cut_high_threshold_pct: _, // frozen()
        floor_owner: _,                // frozen()
        entry_probe: _,                // frozen(), a_surface_probe_reads_the_ops_own_leave
        link_ceiling: _,               // frozen()
        feeds_hints: _,                // the_feeds_hints_read_the_ops_own_fields
        move_count: _,                 // frozen()
        doc_basis: _,                  // frozen()
        air_cut_hint: _,               // frozen()
        relink_dial: _,                // frozen()
        axial_envelope: _,             // frozen()
        rest_use: _,                   // frozen()
        territory_xy: _,               // frozen()
        plans_bands: _,                // frozen_c2()
        ramp_descent: _,               // frozen_c2()
        adaptive_woc: _,               // frozen_c2()
        flat_floor_clearing: _,        // frozen_c2()
        clearing_strategy_dial: _,     // frozen_c2()
        planner_uniform_engagement: _, // frozen_c2()
        top_on_stock_box: _,           // frozen_c2()
        floor_inside_stock: _,         // frozen_c2()
        lut_route: _,                  // frozen_d2()
        item_use: _,                   // frozen_p1()
        ladder_includes_top: _,        // frozen_f033b()
    } = OperationType::Pocket.capabilities();
}
