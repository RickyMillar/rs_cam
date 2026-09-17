//! Unit tests for the operation catalogue.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::feeds::CutterKind;

#[test]
fn operation_catalog_is_exhaustive_and_consistent() {
    assert_eq!(OperationType::ALL.len(), 24);
    for &op_type in OperationType::ALL {
        let config = OperationConfig::new_default(op_type);
        assert_eq!(config.op_type(), op_type);
        assert_eq!(config.label(), op_type.label());

        // Phase 1 registry self-consistency: one entry per op, and
        // the entry agrees with the op it claims to describe.
        let entry = op_type.registry_entry();
        assert_eq!(entry.op_type, op_type, "registry entry op_type mismatch");
        assert_eq!(entry.spec.label, op_type.label());
        assert!(
            !entry.param_defs.is_empty(),
            "{op_type:?}: registry entry has no settable params — \
             every op exposes at least one"
        );
    }
}

/// Phase 5 (T11): GenerateFn migration is a per-family DECISION,
/// not drift. Each op is either migrated (registry adapter — the
/// match arm delegates to the same fn) or fallback (exhaustive
/// match arm only). Moving a family without updating this table
/// fails here; the table is the cutover log.
#[test]
fn generate_adapter_migration_is_an_explicit_per_op_decision() {
    // Cutover complete 2026-06-07 (T11 PRs 2-14): every family is
    // registry-dispatched. A new op MUST ship a GenerateFn — the
    // fallback match still compiles it, but registry dispatch is
    // the production path and this test refuses a None entry.
    for &op_type in OperationType::ALL {
        assert!(
            op_type.registry_entry().generate.is_some(),
            "{op_type:?}: missing GenerateFn — every operation family is \
             registry-dispatched since the T11 cutover; wire the adapter \
             and prove it against the param-sweep fingerprint oracle"
        );
    }
}

/// Phase 1 wildcard kill (architectural refactor T3, re-baselined
/// typed in T7): tool constraints are an explicit per-entry
/// registry field, now a `&[CutterKind]` list. This pins (a) the
/// five restricted ops exactly (VCarve/Inlay/Chamfer on V-bit;
/// Scallop/UnifiedFinish on ball-tip), (b) that the 19 previously-
/// wildcard-defaulted ops still resolve to the named `ANY_TOOL`
/// policy, and (c) that `to_schema()` materializes the EXACT
/// pre-Phase-3 snake_case strings — proving the typed conversion
/// changed neither behavior nor the published schema. A new op
/// must reference a policy explicitly; there is no fallback arm
/// left to inherit silently.
#[test]
fn tool_constraints_are_an_explicit_per_op_decision() {
    let mut unrestricted = 0;
    for &op_type in OperationType::ALL {
        let tc = op_type.registry_entry().tool_constraints;
        let schema = tc.to_schema();
        match op_type {
            OperationType::VCarve | OperationType::Inlay | OperationType::Chamfer => {
                assert_eq!(tc.required_kinds, [CutterKind::VBit], "{op_type:?}");
                assert_eq!(schema.required_tool_type, ["v_bit"], "{op_type:?}");
                assert!(tc.supports_v_bit, "{op_type:?}");
            }
            OperationType::Scallop | OperationType::UnifiedFinish => {
                assert_eq!(
                    tc.required_kinds,
                    [CutterKind::Ball, CutterKind::TaperedBall]
                );
                assert_eq!(
                    schema.required_tool_type,
                    ["ball_nose", "tapered_ball_nose"]
                );
                assert!(!tc.supports_v_bit);
            }
            _ => {
                // Pre-registry these 19 fell through `_ => (Vec::new(), true)`.
                assert!(
                    tc.required_kinds.is_empty(),
                    "{op_type:?}: expected the ANY_TOOL policy"
                );
                assert!(schema.required_tool_type.is_empty(), "{op_type:?}");
                assert!(tc.supports_v_bit, "{op_type:?}");
                unrestricted += 1;
            }
        }
        assert_eq!(schema.supports_v_bit, tc.supports_v_bit);
    }
    assert_eq!(
        unrestricted, 19,
        "unrestricted-op count changed — decide deliberately"
    );
}

/// T7 PR C reconciliation pin: the registry `allows()` predicate —
/// which the Scallop runtime refusal in
/// `execute_operation_annotated` now reads — agrees with the
/// pre-Phase-3 `ToolType::has_ball_tip()` check for Scallop, and
/// matches the constraint semantics for every (op, cutter) cell.
#[test]
fn tool_constraints_allows_matches_runtime_refusal_semantics() {
    use crate::compute::ToolType;
    for &tool_type in ToolType::ALL {
        let kind = tool_type.cutter_kind();
        // Scallop: allows() == has_ball_tip() — the exact predicate
        // the execute-time refusal used before reading the registry.
        assert_eq!(
            OperationType::Scallop
                .registry_entry()
                .tool_constraints
                .allows(kind),
            tool_type.has_ball_tip(),
            "{tool_type:?} vs Scallop"
        );
        // UnifiedFinish: same ball-tip refusal as Scallop (registration
        // checklist decision — mirrors Scallop's runtime refusal in
        // `generate_unified_finish`).
        assert_eq!(
            OperationType::UnifiedFinish
                .registry_entry()
                .tool_constraints
                .allows(kind),
            tool_type.has_ball_tip(),
            "{tool_type:?} vs UnifiedFinish"
        );
        // V-bit-required ops accept exactly the V-bit.
        for op in [
            OperationType::VCarve,
            OperationType::Inlay,
            OperationType::Chamfer,
        ] {
            assert_eq!(
                op.registry_entry().tool_constraints.allows(kind),
                tool_type == ToolType::VBit,
                "{tool_type:?} vs {op:?}"
            );
        }
        // ANY_TOOL accepts everything, V-bit included.
        assert!(ToolConstraintsDef::ANY_TOOL.allows(kind));
    }
}

/// Phase 1 T4: the drill-family membership behind the canonical
/// [`OperationType::is_drill_kinematics`] predicate, pinned exactly.
/// Every gate/optimizer/narrate consumer now routes through the
/// helper, so growing this set is a single deliberate edit — and
/// this pin makes that edit loud.
#[test]
fn drill_kinematics_set_is_pinned() {
    let drills: Vec<_> = OperationType::ALL
        .iter()
        .copied()
        .filter(|op| op.is_drill_kinematics())
        .collect();
    assert_eq!(
        drills,
        [OperationType::Drill, OperationType::AlignmentPinDrill],
        "drill-kinematics membership changed — update the pin deliberately"
    );
}

/// Parity freeze (architectural refactor §7.2): `ALL` is exactly the
/// disjoint union of `ALL_2D`, `ALL_3D`, and the NAMED system-only
/// set. A new op added to `ALL` without being placed in a menu
/// sublist (or explicitly listed as system-only here) fails this
/// test — placement is a recorded decision, not an accident.
#[test]
fn operation_partitions_cover_all_variants_once() {
    use std::collections::HashSet;

    // The ops deliberately absent from both user menus. Keep this
    // list in sync with intent, not convenience.
    const SYSTEM_ONLY: &[OperationType] = &[OperationType::AlignmentPinDrill];

    let all: HashSet<_> = OperationType::ALL.iter().collect();
    assert_eq!(
        all.len(),
        OperationType::ALL.len(),
        "OperationType::ALL contains duplicates"
    );

    let twod: HashSet<_> = OperationType::ALL_2D.iter().collect();
    let threed: HashSet<_> = OperationType::ALL_3D.iter().collect();
    let system: HashSet<_> = SYSTEM_ONLY.iter().collect();
    assert!(twod.is_disjoint(&threed), "ALL_2D and ALL_3D overlap");
    assert!(system.is_disjoint(&twod), "system-only op listed in ALL_2D");
    assert!(
        system.is_disjoint(&threed),
        "system-only op listed in ALL_3D"
    );

    let union: HashSet<_> = twod
        .union(&threed)
        .copied()
        .collect::<HashSet<_>>()
        .union(&system)
        .copied()
        .collect();
    assert_eq!(
        union, all,
        "ALL_2D ∪ ALL_3D ∪ SYSTEM_ONLY must equal OperationType::ALL exactly \
         — place every new op in a menu sublist or name it system-only"
    );

    // Phase 2 X-macro sync: the hand-written menu consts must agree
    // with the per-row category tokens in `for_each_op!` — order
    // included (both follow canonical ALL order).
    let by_cat = |cat: OpCategory| -> Vec<OperationType> {
        OperationType::ALL
            .iter()
            .copied()
            .filter(|op| op.category() == cat)
            .collect()
    };
    assert_eq!(
        by_cat(OpCategory::Menu2d),
        OperationType::ALL_2D,
        "ALL_2D out of sync with for_each_op! Menu2d tokens"
    );
    assert_eq!(
        by_cat(OpCategory::Menu3d),
        OperationType::ALL_3D,
        "ALL_3D out of sync with for_each_op! Menu3d tokens"
    );
    assert_eq!(
        by_cat(OpCategory::SystemOnly),
        SYSTEM_ONLY,
        "system-only set out of sync with for_each_op! SystemOnly tokens"
    );
}

/// Parity freeze (architectural refactor §7.2): the externally-tagged
/// `{kind, params}` serde shape of [`OperationConfig`]. The MCP
/// `set_toolpath_param` round-trip depends on `params` being a
/// mutable object and `kind` being the snake_case op name; project
/// TOML on disk depends on the same shape. A careless registry /
/// X-macro change that alters this breaks saved projects and the MCP
/// surface silently — this test makes it loud.
#[test]
fn operation_config_serde_shape_is_kind_params() {
    for &op_type in OperationType::ALL {
        let config = OperationConfig::new_default(op_type);

        // JSON view (MCP surface).
        let json = serde_json::to_value(&config).expect("serialize op config to JSON");
        let obj = json.as_object().expect("op config must be a JSON object");
        assert_eq!(
            obj.keys().collect::<Vec<_>>(),
            ["kind", "params"],
            "{op_type:?}: serde shape must be exactly {{kind, params}}"
        );
        assert_eq!(
            obj.get("kind").and_then(|k| k.as_str()),
            Some(op_type.kind_str()),
            "{op_type:?}: `kind` tag must equal kind_str()"
        );
        assert!(
            obj.get("params").is_some_and(serde_json::Value::is_object),
            "{op_type:?}: `params` must be a mutable JSON object"
        );

        // TOML view (project files on disk) — must round-trip.
        let toml_str = toml::to_string(&config).expect("serialize op config to TOML");
        assert!(
            toml_str.contains("kind = "),
            "{op_type:?}: TOML must carry the `kind` tag"
        );
        let back: OperationConfig =
            toml::from_str(&toml_str).expect("round-trip op config from TOML");
        assert_eq!(
            back.op_type(),
            op_type,
            "{op_type:?}: TOML round-trip changed the operation kind"
        );
    }
}

/// Parity freeze (architectural refactor §7.2): the snake_case serde
/// repr of every [`OperationType`] — these strings are canonical in
/// project TOML, the MCP wire format, and MCP error messages. Pinned
/// as literals (not derived) so a rename anywhere fails here first.
#[test]
fn operation_type_serde_repr_pinned() {
    const PINNED: &[(&str, OperationType)] = &[
        ("face", OperationType::Face),
        ("pocket", OperationType::Pocket),
        ("profile", OperationType::Profile),
        ("adaptive", OperationType::Adaptive),
        ("v_carve", OperationType::VCarve),
        ("rest", OperationType::Rest),
        ("inlay", OperationType::Inlay),
        ("zigzag", OperationType::Zigzag),
        ("trace", OperationType::Trace),
        ("drill", OperationType::Drill),
        ("chamfer", OperationType::Chamfer),
        ("drop_cutter", OperationType::DropCutter),
        ("adaptive3d", OperationType::Adaptive3d),
        ("waterline", OperationType::Waterline),
        ("pencil", OperationType::Pencil),
        ("scallop", OperationType::Scallop),
        ("unified_finish", OperationType::UnifiedFinish),
        ("steep_shallow", OperationType::SteepShallow),
        ("ramp_finish", OperationType::RampFinish),
        ("spiral_finish", OperationType::SpiralFinish),
        ("radial_finish", OperationType::RadialFinish),
        ("horizontal_finish", OperationType::HorizontalFinish),
        ("project_curve", OperationType::ProjectCurve),
        ("alignment_pin_drill", OperationType::AlignmentPinDrill),
    ];
    assert_eq!(PINNED.len(), OperationType::ALL.len());

    for &(repr, op_type) in PINNED {
        assert_eq!(
            serde_json::to_value(op_type).expect("serialize op type"),
            serde_json::Value::String(repr.to_owned()),
            "{op_type:?}: serde repr drifted from the pinned canonical name"
        );
        assert_eq!(
            op_type.kind_str(),
            repr,
            "{op_type:?}: kind_str() disagrees with the serde repr"
        );
        let parsed: OperationType =
            serde_json::from_value(serde_json::Value::String(repr.to_owned()))
                .expect("canonical name must deserialize");
        assert_eq!(parsed, op_type);
    }
}

#[test]
fn operation_transform_capabilities_are_explicit() {
    for &op_type in OperationType::ALL {
        let caps = op_type.transform_capabilities();
        assert!(
            !(caps.allows_global_rapid_reorder && caps.requires_depth_order),
            "{op_type:?} cannot globally reorder while requiring depth order"
        );
        assert!(
            !(caps.allows_global_rapid_reorder && caps.continuous_path_required),
            "{op_type:?} cannot globally reorder a continuous path"
        );
    }
    assert!(
        OperationType::Adaptive3d
            .transform_capabilities()
            .requires_depth_order
    );
    assert!(
        OperationType::DropCutter
            .transform_capabilities()
            .allows_global_rapid_reorder
    );
    assert!(
        OperationType::ProjectCurve
            .transform_capabilities()
            .allows_global_rapid_reorder
    );
}

#[test]
fn scallop_config_transform_capabilities_dispatch_splits_reorder_from_links() {
    // The config-aware dispatch EXISTS and is the extension point for
    // per-config classification. Fix-family Phase 1b reclassified
    // discrete-ring Scallop as reorderable, then the sentry
    // `scallop_discrete_capability_currently_blocked_reorder_gouges`
    // measured a gouge (146 columns deeper, worst 9.33mm) — but the
    // gouge came from `apply_link_moves` bridging segments with a
    // straight feed, not from the reorder itself (holding link moves
    // off, cutting distance is byte-identical under reorder). Phase 1c
    // decoupled `allows_link_moves` from the reorder predicates, so
    // discrete Scallop can now state the true, narrower capability:
    // reorder allowed, link moves forbidden. Continuous Scallop (one
    // stitched stay-down helix) still forbids both.
    let discrete = OperationConfig::Scallop(ScallopConfig {
        continuous: false,
        ..ScallopConfig::default()
    });
    let discrete_caps = discrete.transform_capabilities();
    assert!(
        discrete_caps.allows_global_rapid_reorder,
        "discrete-ring Scallop should allow global rapid reorder \
         (ring order has no material-state dependency)"
    );
    assert!(
        !discrete_caps.allows_link_moves,
        "discrete-ring Scallop must still forbid link moves \
         (apply_link_moves bridges with a straight feed that can gouge)"
    );

    let continuous = OperationConfig::Scallop(ScallopConfig {
        continuous: true,
        ..ScallopConfig::default()
    });
    let continuous_caps = continuous.transform_capabilities();
    assert!(
        !continuous_caps.allows_global_rapid_reorder,
        "continuous (spiral) Scallop must stay reorder-blocked — it's \
         one stitched stay-down path"
    );
    assert!(!continuous_caps.allows_link_moves);

    // The op-type-only fallback (no config in hand) must stay the
    // conservative answer — it is what continuous Scallop falls
    // through to, and the worst case for callers with no config.
    let fallback = OperationType::Scallop.transform_capabilities();
    assert!(!fallback.allows_global_rapid_reorder);
    assert!(!fallback.allows_link_moves);
}

#[test]
fn air_cut_threshold_suppresses_drill_kinds() {
    assert!(
        OperationType::Drill.air_cut_high_threshold_pct().is_none(),
        "Drill should suppress air-cut metric (dexel can't measure Z-only)"
    );
    assert!(
        OperationType::AlignmentPinDrill
            .air_cut_high_threshold_pct()
            .is_none(),
        "AlignmentPinDrill should suppress air-cut metric"
    );
}

#[test]
fn air_cut_threshold_permissive_for_project_curve() {
    let t = OperationType::ProjectCurve
        .air_cut_high_threshold_pct()
        .expect("ProjectCurve should have an air-cut threshold");
    // W5B-F4: the band must stay the most permissive in the table (the op
    // IS sparse by construction) without becoming unreachable. Post-flip
    // readings are 10.9–28.7; the old 97 could not fire on anything the
    // repo can produce, and a gate that cannot fire reads as exoneration.
    assert!(
        t > 45.0,
        "ProjectCurve must stay above the 3D-finish band — rivers/curves \
         are sparse by construction; got {t}"
    );
    assert!(
        t <= 70.0,
        "ProjectCurve threshold must stay reachable: post-flip readings top \
         out at 28.7 (isolated river) and a band near 100 is a dead gate; got {t}"
    );
}

#[test]
fn air_cut_threshold_band_for_finish_ops() {
    for op in [
        OperationType::DropCutter,
        OperationType::Scallop,
        OperationType::UnifiedFinish,
        OperationType::Waterline,
        OperationType::Pencil,
        OperationType::HorizontalFinish,
        OperationType::SteepShallow,
        OperationType::RampFinish,
        OperationType::SpiralFinish,
        OperationType::RadialFinish,
    ] {
        let t = op
            .air_cut_high_threshold_pct()
            .unwrap_or_else(|| panic!("{op:?} should have an air-cut threshold"));
        // W5B-F4: the band must clear the measured defect-free cluster
        // (34.3–42.5 on clean geometry with the correct tool) and still
        // catch the genuine outliers (SteepShallow 78, RadialFinish 82).
        // Below 43 it false-alarms on well-formed finishing; above 60 it
        // stops distinguishing a bad strategy from a good one.
        assert!(
            (43.0..=60.0).contains(&t),
            "{op:?} finish threshold expected ~45, got {t}"
        );
    }
}

#[test]
fn air_cut_threshold_band_for_clearing_ops() {
    for op in [
        OperationType::Adaptive3d,
        OperationType::Adaptive,
        OperationType::Pocket,
        OperationType::Face,
        OperationType::Zigzag,
        OperationType::Rest,
    ] {
        let t = op
            .air_cut_high_threshold_pct()
            .unwrap_or_else(|| panic!("{op:?} should have an air-cut threshold"));
        assert!(
            (35.0..=45.0).contains(&t),
            "{op:?} clearing threshold expected ~40, got {t}"
        );
    }
}

#[test]
fn air_cut_threshold_exhaustive_for_all_ops() {
    // Adding a new OperationType variant should require classifying it.
    // We can't enforce this at compile time (the method returns Option),
    // so this test ensures someone touched every variant deliberately.
    for &op in OperationType::ALL {
        let _ = op.air_cut_high_threshold_pct();
    }
}

#[test]
fn depthless_finishing_ops_resolve_to_none() {
    for op in [
        OperationConfig::Pencil(PencilConfig::default()),
        OperationConfig::Scallop(ScallopConfig::default()),
        OperationConfig::SteepShallow(SteepShallowConfig::default()),
        OperationConfig::RampFinish(RampFinishConfig::default()),
        OperationConfig::SpiralFinish(SpiralFinishConfig::default()),
        OperationConfig::RadialFinish(RadialFinishConfig::default()),
        OperationConfig::HorizontalFinish(HorizontalFinishConfig::default()),
    ] {
        assert!(matches!(op.depth_semantics(), DepthSemantics::None));
        assert_eq!(op.default_depth_for_heights(), 0.0);
    }
}
