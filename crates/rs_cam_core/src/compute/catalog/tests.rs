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
        // CMP-01: every row names a generation adapter. The field is
        // `GenerateFn`, not `Option<GenerateFn>`, since the T11 fallback
        // match was deleted, so this reads as a type check rather than a
        // runtime one — which is why
        // `generate_adapter_migration_is_an_explicit_per_op_decision`
        // went with the match it guarded.
        let _: crate::compute::execute::GenerateFn = entry.generate;
    }
}

/// CMP-10: every field of an operation's config is either a published
/// `ParamDef` or a named exemption with a reason.
///
/// Nothing compared the two tables before this. The gap ran both ways:
/// a new struct field was simply invisible to MCP, and
/// `params_value_including_nulls` inserts a JSON null for any `ParamDef`
/// name, so a def naming a field that no longer exists would publish a
/// null for ever. CMP-09 found two live instances by hand;
/// `DropCutterConfig::scallop_height` — a dial the GUI could turn and an
/// agent could not — is now published, and the other is recorded below.
///
/// # Why the field list comes from the SOURCE
///
/// The obvious instrument is the serialised default config. It does not
/// work, and the way it fails is the reason CMP-09 went unseen: almost
/// every unexposed field is an `Option` with
/// `skip_serializing_if = "Option::is_none"`, so a default config does
/// not serialise it at all. `scallop_height` is exactly that shape. An
/// instrument built on the default's key set reports a clean sweep over
/// an empty population.
///
/// So this reads `operation_configs.rs` and takes each config struct's
/// own `pub` field list. The field counts below are the non-vacuity
/// anchor: the parser must find all 24 structs and a field total in the
/// right order of magnitude, or it is measuring nothing.
#[test]
fn param_defs_cover_every_config_field() {
    /// One entry per field that exists and is deliberately NOT settable
    /// through `set_toolpath_param`. Every entry carries its reason.
    /// Adding a row here is a decision; the absence of a row is a defect.
    const UNEXPOSED: &[(&str, &str)] = &[
        (
            "selected_holes",
            "driver-set: the DXF/model drill picker writes the picked target list, on \
             DrillConfig and AlignmentPinDrillConfig alike",
        ),
        (
            "selected_layers",
            "driver-set: the layer-select route writes this beside the picked list",
        ),
        (
            "setup_z_flipped",
            "driver-set: the setup transform stamps this, not the operator",
        ),
    ];

    const CONFIG_SRC: &str = include_str!("../operation_configs.rs");

    /// Every `pub` field of one config struct, read from the source.
    fn fields_of(struct_name: &str) -> Vec<&'static str> {
        let head = format!("pub struct {struct_name} {{");
        let at = CONFIG_SRC
            .find(&head)
            .unwrap_or_else(|| panic!("`{head}` is not in operation_configs.rs"));
        let body_start = at + head.len();
        // Every struct in this file closes with a `}` in column 0.
        let body_len = CONFIG_SRC[body_start..]
            .find("\n}")
            .unwrap_or_else(|| panic!("`{struct_name}` has no closing brace"));
        CONFIG_SRC[body_start..body_start + body_len]
            .lines()
            .filter_map(|line| {
                let t = line.trim_start();
                let rest = t.strip_prefix("pub ")?;
                let colon = rest.find(':')?;
                let name = &rest[..colon];
                name.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    .then_some(name)
            })
            .collect()
    }

    let mut unexposed_seen: Vec<&str> = Vec::new();
    let mut field_total = 0;
    let mut def_total = 0;

    for &op_type in OperationType::ALL {
        let struct_name = format!("{}Config", op_type.name());
        let fields = fields_of(&struct_name);
        assert!(
            !fields.is_empty(),
            "{struct_name}: the parser found no fields — it is measuring nothing"
        );
        field_total += fields.len();

        let published = OperationConfig::param_names_for_type(op_type);
        def_total += op_type.registry_entry().param_defs.len();

        for field in &fields {
            if published.contains(field) {
                continue;
            }
            assert!(
                UNEXPOSED.iter().any(|(name, _)| name == field),
                "{op_type:?}: `{struct_name}::{field}` is in no `param_defs` array, so no \
                 agent can set it and `get_operation_schema` does not mention it. Publish \
                 it as a `ParamDef`, or add it to this test's UNEXPOSED list with the \
                 reason it is driver-set."
            );
            unexposed_seen.push(field);
        }

        // The other direction, as far as it can honestly go: a def must
        // name a field of the struct it describes.
        for def in op_type.registry_entry().param_defs {
            assert!(
                fields.contains(&def.name),
                "{op_type:?}: `param_defs` publishes `{}`, and `{struct_name}` has no field \
                 of that name. The schema advertises a dial that writes nothing, and \
                 `params_value_including_nulls` publishes a null for it for ever.",
                def.name
            );
        }
    }

    // Non-vacuity anchors. The audit of 2026-09-17 counted 243 defs
    // against 250 fields; these bars are deliberately loose, because the
    // claim is "the parser read the real tables", not a pinned census.
    assert!(
        field_total > 200,
        "the parser found only {field_total} config fields over 24 structs"
    );
    assert!(
        def_total > 200,
        "the registry holds only {def_total} param defs over 24 operations"
    );

    // Non-vacuity on the allow-list itself: an entry nobody hits is a
    // stale exemption, and a stale exemption hides the next real one.
    for (name, _) in UNEXPOSED {
        assert!(
            unexposed_seen.contains(name),
            "the UNEXPOSED entry `{name}` matches no config field any more. Delete the row."
        );
    }
}

/// CMP-08: every alias the registry publishes names a real def on the
/// same operation, and no alias collides with a published name.
#[test]
fn every_alias_names_a_field_of_its_own_operation() {
    let mut aliases_seen = 0;
    for &op_type in OperationType::ALL {
        let entry = op_type.registry_entry();
        for def in entry.param_defs {
            for alias in def.aliases {
                aliases_seen += 1;
                assert!(
                    entry.param_defs.iter().all(|other| other.name != *alias),
                    "{op_type:?}: `{alias}` is both an alias of `{}` and a param name of                      its own. One name, one field.",
                    def.name
                );
            }
        }
        for name in OperationConfig::param_names_for_type(op_type) {
            assert!(
                OperationConfig::new_default(op_type)
                    .param_type_name(name)
                    .is_some(),
                "{op_type:?}: the published name `{name}` resolves to no def"
            );
        }
    }
    // The three the finding names: Waterline and RampFinish alias
    // `depth_per_pass`, Pencil aliases `stepover`.
    assert_eq!(
        aliases_seen, 3,
        "the alias population moved. Three alias setters exist in          `session/compute/params.rs`; each one needs its registry row."
    );
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

/// CMP-04: `cutting_levels` decides per operation, and the decision
/// agrees with the operation's own declared depth semantics.
///
/// The exhaustiveness half is the compiler's: the match named every
/// variant when the `_ => vec![]` wildcard went, so a 25th operation does
/// not compile until it decides. This test carries the consistency half —
/// an operation produces depth-stepped levels EXACTLY when it declares an
/// explicit total depth AND a per-pass step. An op that declares both and
/// returns no levels would cut at one Z and look generated, which is the
/// silent failure the wildcard used to allow.
#[test]
fn cutting_levels_is_exhaustive_per_op() {
    for &op_type in OperationType::ALL {
        let config = OperationConfig::new_default(op_type);
        let params = config.as_params();
        let steps_down = matches!(params.depth_semantics(), DepthSemantics::Explicit(_))
            && params.depth_per_pass().is_some();
        let levels = config.cutting_levels(0.0);
        assert_eq!(
            !levels.is_empty(),
            steps_down,
            "{op_type:?}: cutting_levels returned {} level(s) but its declared \
             depth semantics say steps_down = {steps_down}",
            levels.len()
        );
    }
}

/// CMP-07: the lateral-raster membership set, pinned like the drill set.
///
/// It was `matches!(self, DropCutter | SteepShallow | SpiralFinish |
/// HorizontalFinish)` in `catalog.rs`, which fails OPEN: a 25th operation
/// joins the false side without a compiler word. The set is a registry row
/// field now; this pins what it contains.
#[test]
fn lateral_raster_stepover_set_is_pinned() {
    let lateral: Vec<_> = OperationType::ALL
        .iter()
        .copied()
        .filter(|op| op.lateral_raster_stepover())
        .collect();
    assert_eq!(
        lateral,
        [
            OperationType::DropCutter,
            OperationType::SteepShallow,
            OperationType::SpiralFinish,
            OperationType::HorizontalFinish,
        ],
        "lateral-raster membership changed — update the pin deliberately"
    );
}

/// CMP-07: the empty-generation exemption set, pinned like the drill set.
///
/// It was `matches!(op_type, Pencil | HorizontalFinish | Waterline)` in
/// `generated_empty.rs`. Both directions of a silent change hurt: an op
/// that should be exempt and is not reads as a spurious refusal, and an op
/// that should be gated and is not lets a failed generation pass as an
/// absent feature.
#[test]
fn feature_selective_exemption_set_is_pinned() {
    let exempt: Vec<_> = OperationType::ALL
        .iter()
        .copied()
        .filter(|op| crate::compute::generated_empty::feature_selective_exemption(*op))
        .collect();
    assert_eq!(
        exempt,
        [
            OperationType::Waterline,
            OperationType::Pencil,
            OperationType::HorizontalFinish,
        ],
        "empty-generation exemption set changed — update the pin deliberately"
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

// ── UI-04: every dial the GUI keys on states its help ────────────────────
//
// The GUI tooltip used to be a 60-arm match on the visible LABEL string
// (`viz/ui/properties/linking_dressup.rs::tooltip_for`). `"Stepover:"`
// reached its help only because two spellings agreed by hand, so re-wording
// a label dropped the tooltip and nothing failed. The key is now
// `(OperationType, param name)` and the text is `ParamDef::help`, which is
// also what `operation_schema` serves an agent as `description`.
//
// This arm is a RATCHET, not a ban. 127 of the 244 rows state no help yet,
// and writing those needs the person who knows each dial — the same sweep
// `SYNTHESIS.md` deferred for CMP-05's ranges. The number below only ever
// goes down.

/// How many `ParamDef` rows may still state no help.
///
/// | Package | Budget | Note |
/// |---|---|---|
/// | UI-04 | 127 | measured after the 60 GUI tooltips moved onto their rows |
const HELPLESS_PARAM_BUDGET: usize = 127;

#[test]
fn param_def_help_only_ever_grows_ui04() {
    let mut helpless = Vec::new();
    let mut total = 0usize;
    for &op in OperationType::ALL {
        for def in op.registry_entry().param_defs {
            total += 1;
            match def.help {
                Some(text) => assert!(
                    !text.trim().is_empty(),
                    "{op:?}.{} states an EMPTY help string. An empty string is \
                     not an abstention; write the line or leave the field None.",
                    def.name
                ),
                None => helpless.push(format!("{op:?}.{}", def.name)),
            }
        }
    }
    assert!(total > 200, "the registry shrank to {total} params");
    assert!(
        helpless.len() <= HELPLESS_PARAM_BUDGET,
        "{} ParamDef rows state no help, over the budget of \
         {HELPLESS_PARAM_BUDGET}. Lower the budget in the package that \
         writes the lines; never raise it. First few: {:?}",
        helpless.len(),
        helpless.iter().take(8).collect::<Vec<_>>()
    );
}

/// The GUI resolves its tooltip through this exact path, so a parameter
/// name that no row carries returns `None` rather than panicking. The viz
/// sentry `the_help_key_is_the_registry_param_name_ui04` holds the GUI's
/// keys against this table.
#[test]
fn a_param_name_resolves_to_at_most_one_row_ui04() {
    for &op in OperationType::ALL {
        let defs = op.registry_entry().param_defs;
        for def in defs {
            let hits = defs.iter().filter(|d| d.name == def.name).count();
            assert_eq!(
                hits, 1,
                "{op:?} lists `{}` {hits} times. A duplicated name makes the \
                 help lookup pick an arbitrary row.",
                def.name
            );
        }
    }
}
