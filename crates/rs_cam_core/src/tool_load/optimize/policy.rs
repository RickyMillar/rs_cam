//! Optimizer search policy with provenance for every tuning constant.
//!
//! Step 1 of G16 keeps optimizer behaviour identical while moving the
//! constants out of `optimize.rs` and giving each value an owner, rationale,
//! and source. Later refactor steps will thread `SearchPolicy` explicitly; for
//! now callers read [`SearchPolicy::default`] at the use site so this extraction
//! stays behaviour-preserving.

/// A tunable policy value plus the provenance needed to review it.
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyValue<T> {
    pub value: T,
    pub rationale: &'static str,
    pub source: PolicySource,
}

/// Where a policy value came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicySource {
    /// Physical / machine / safety limit. Don't change without re-deriving the
    /// analysis.
    PhysicalLimit,
    /// Empirical handbook or project engineering default.
    Handbook { citation: &'static str },
    /// Tuning choice expected to move as real project data accumulates.
    TuningChoice { hypothesis: &'static str },
    /// Derived from another model or subsystem rather than chosen directly.
    Derived { from: &'static str },
}

/// Complete optimizer tuning posture.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchPolicy {
    pub axes: AxesPolicy,
    pub feed: FeedPolicy,
    pub retarget: RetargetPolicy,
    pub ranking: RankingPolicy,
    pub stages: StagePolicy,
    pub fallback: FallbackPolicy,
    pub deflection_setup_target_um: PolicyValue<f64>,
    pub bottleneck_fraction: PolicyValue<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AxesPolicy {
    pub doc: AxisPolicy,
    pub stepover: AxisPolicy,
    pub scallop_height: AxisPolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AxisPolicy {
    pub baseline_mult_lo: PolicyValue<f64>,
    pub baseline_mult_hi_three_point: PolicyValue<f64>,
    pub baseline_mult_hi_four_point: PolicyValue<f64>,
    pub three_point_count: PolicyValue<usize>,
    pub four_point_count: PolicyValue<usize>,
    pub midpoint_weight: PolicyValue<f64>,
    pub hard_floor: PolicyValue<f64>,
    pub hard_ceiling: PolicyValue<f64>,
    pub dedup_tolerance: PolicyValue<f64>,
    pub allow_outside_preferred: PolicyValue<bool>,
    pub outside_preferred_hi_mult: PolicyValue<f64>,
    pub outside_preferred_lo_mult: PolicyValue<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeedPolicy {
    pub min_positive_scale_input: PolicyValue<f64>,
    pub dedup_threshold_fraction: PolicyValue<f64>,
    pub plunge_tracking_threshold_fraction: PolicyValue<f64>,
    pub delta_display_tolerance_mm_min: PolicyValue<f64>,
    pub scale_floor: PolicyValue<f64>,
    pub scale_epsilon: PolicyValue<f64>,
    pub lut_rpm_nominal_headroom: PolicyValue<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetargetPolicy {
    pub chipload_target_midpoint_weight: PolicyValue<f64>,
    pub rpm_bracket_midpoint_weight: PolicyValue<f64>,
    pub chipload_upper_only_fraction: PolicyValue<f64>,
    pub chipload_lower_only_headroom: PolicyValue<f64>,
    pub chipload_low_headroom: PolicyValue<f64>,
    pub chipload_high_headroom: PolicyValue<f64>,
    pub power_headroom: PolicyValue<f64>,
    pub deflection_headroom: PolicyValue<f64>,
    pub extrapolation_approx_ln_threshold: PolicyValue<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankingPolicy {
    pub recommendation_cycle_delta_s: PolicyValue<f64>,
    pub failing_gate_relative_threshold: PolicyValue<f64>,
    pub failing_gate_absolute_epsilon: PolicyValue<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StagePolicy {
    pub coarse_resolution_mm: PolicyValue<f64>,
    pub refined_resolution_mm: PolicyValue<f64>,
    pub refined_survivor_count: PolicyValue<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FallbackPolicy {
    pub doc_anchor_mm: PolicyValue<f64>,
    pub stepover_anchor_mm: PolicyValue<f64>,
    pub scallop_height_anchor_mm: PolicyValue<f64>,
}

impl Default for SearchPolicy {
    fn default() -> Self {
        Self {
            axes: AxesPolicy {
                doc: AxisPolicy {
                    baseline_mult_lo: PolicyValue {
                        value: 0.7,
                        rationale: "Lower bound for local DOC sweeps when calibrated bounds are absent or narrower than the warm-start envelope.",
                        source: PolicySource::Handbook {
                            citation: "Engineering Default 9, optimizer redesign 2026-05-08",
                        },
                    },
                    baseline_mult_hi_three_point: PolicyValue {
                        value: 1.3,
                        rationale: "Upper bound for three-point DOC sweeps: low, baseline, high without adding extra sim cost.",
                        source: PolicySource::Handbook {
                            citation: "Engineering Default 9, optimizer redesign 2026-05-08",
                        },
                    },
                    baseline_mult_hi_four_point: PolicyValue {
                        value: 1.4,
                        rationale: "Upper bound for four-point clearing sweeps; midpoint between baseline and high lands at 1.2x.",
                        source: PolicySource::Handbook {
                            citation: "Engineering Default 9, optimizer redesign 2026-05-08",
                        },
                    },
                    three_point_count: PolicyValue {
                        value: 3,
                        rationale: "Three-point geometry sweep shape: low, baseline, high.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Three sims are enough for lower-leverage geometry axes.",
                        },
                    },
                    four_point_count: PolicyValue {
                        value: 4,
                        rationale: "Four-point clearing sweep shape: low, baseline, midpoint, high.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Pocket/adaptive axes justify one extra sim for a midpoint candidate.",
                        },
                    },
                    midpoint_weight: PolicyValue {
                        value: 0.5,
                        rationale: "Four-point DOC sweeps place the extra candidate halfway between baseline and high endpoint.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "The midpoint catches useful moderate changes without a dense grid.",
                        },
                    },
                    hard_floor: PolicyValue {
                        value: 0.05,
                        rationale: "Sub-50um DOC in wood is effectively rubbing/polishing, outside the roughing optimizer envelope.",
                        source: PolicySource::PhysicalLimit,
                    },
                    hard_ceiling: PolicyValue {
                        value: 100.0,
                        rationale: "Sane upper cap on the DOC hard envelope, well above any router/wood scenario; tool cutting length may tighten this in retargeters.",
                        source: PolicySource::PhysicalLimit,
                    },
                    dedup_tolerance: PolicyValue {
                        value: 0.005,
                        rationale: "5um differences are below simulator-distinguishable resolution for this search stage.",
                        source: PolicySource::Derived {
                            from: "tri-dexel simulation resolution",
                        },
                    },
                    allow_outside_preferred: PolicyValue {
                        value: true,
                        rationale: "Future bounds resolver may probe beyond LUT-preferred bounds because every candidate is sim-verified.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Aggressive probes recover baselines that drift outside vendor envelopes without compromising safety gates.",
                        },
                    },
                    outside_preferred_hi_mult: PolicyValue {
                        value: 1.15,
                        rationale: "Probe just above LUT-preferred max at +15% to challenge vendor envelope without leaping outside hard limits.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "+15% catches the wanaka TP 4 case where the user's intent is well above LUT ae_max.",
                        },
                    },
                    outside_preferred_lo_mult: PolicyValue {
                        value: 0.85,
                        rationale: "Probe just below LUT-preferred min at -15% so chipload-low retargets aren't trapped at vendor floor.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Symmetric to the above probe; sim verifies safety.",
                        },
                    },
                },
                stepover: AxisPolicy {
                    baseline_mult_lo: PolicyValue {
                        value: 0.7,
                        rationale: "Lower bound for local stepover sweeps when calibrated bounds are absent or narrower than the warm-start envelope.",
                        source: PolicySource::Handbook {
                            citation: "Engineering Default 9, optimizer redesign 2026-05-08",
                        },
                    },
                    baseline_mult_hi_three_point: PolicyValue {
                        value: 1.3,
                        rationale: "Upper bound for three-point stepover sweeps: low, baseline, high.",
                        source: PolicySource::Handbook {
                            citation: "Engineering Default 9, optimizer redesign 2026-05-08",
                        },
                    },
                    baseline_mult_hi_four_point: PolicyValue {
                        value: 1.4,
                        rationale: "Upper bound for four-point clearing sweeps; midpoint between baseline and high lands at 1.2x.",
                        source: PolicySource::Handbook {
                            citation: "Engineering Default 9, optimizer redesign 2026-05-08",
                        },
                    },
                    three_point_count: PolicyValue {
                        value: 3,
                        rationale: "Three-point stepover sweep shape: low, baseline, high.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Three sims are enough for lower-leverage stepover axes.",
                        },
                    },
                    four_point_count: PolicyValue {
                        value: 4,
                        rationale: "Four-point clearing sweep shape: low, baseline, midpoint, high.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Pocket/adaptive axes justify one extra sim for a midpoint candidate.",
                        },
                    },
                    midpoint_weight: PolicyValue {
                        value: 0.5,
                        rationale: "Four-point stepover sweeps place the extra candidate halfway between baseline and high endpoint.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "The midpoint catches useful moderate changes without a dense grid.",
                        },
                    },
                    hard_floor: PolicyValue {
                        value: 0.05,
                        rationale: "Sub-50um stepover is finishing/polishing territory, not a router roughing search candidate.",
                        source: PolicySource::PhysicalLimit,
                    },
                    hard_ceiling: PolicyValue {
                        value: 100.0,
                        rationale: "Sane upper cap on the stepover hard envelope; well above any practical router stepover value.",
                        source: PolicySource::PhysicalLimit,
                    },
                    dedup_tolerance: PolicyValue {
                        value: 0.005,
                        rationale: "5um differences are below simulator-distinguishable resolution for this search stage.",
                        source: PolicySource::Derived {
                            from: "tri-dexel simulation resolution",
                        },
                    },
                    allow_outside_preferred: PolicyValue {
                        value: true,
                        rationale: "Future bounds resolver may probe beyond LUT-preferred bounds because every candidate is sim-verified.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Aggressive probes recover baselines that drift outside vendor envelopes without compromising safety gates.",
                        },
                    },
                    outside_preferred_hi_mult: PolicyValue {
                        value: 1.15,
                        rationale: "Probe just above LUT-preferred max at +15% so the search exceeds LUT ae_max when the operator's intent demands it.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "+15% recovers the wanaka TP 4 stepover case (LUT ae_max=0.95 vs operator wanting >2.5).",
                        },
                    },
                    outside_preferred_lo_mult: PolicyValue {
                        value: 0.85,
                        rationale: "Probe just below LUT-preferred min at -15%; sim verifies whether chipload still survives.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Symmetric to the above probe.",
                        },
                    },
                },
                scallop_height: AxisPolicy {
                    baseline_mult_lo: PolicyValue {
                        value: 0.7,
                        rationale: "Lower bound for scallop-height quality target sweeps around the user's current finish target.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "A 30% tighter finish target is a visible quality/runtime trade-off without exploding path length.",
                        },
                    },
                    baseline_mult_hi_three_point: PolicyValue {
                        value: 1.3,
                        rationale: "Upper bound for loosening scallop-height quality target around baseline.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "A 30% looser target gives useful runtime probes while preserving operator intent.",
                        },
                    },
                    baseline_mult_hi_four_point: PolicyValue {
                        value: 1.3,
                        rationale: "Scallop height currently uses only a three-point quality-target sweep.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Extra scallop candidates are not worth the sim cost until fixture data says otherwise.",
                        },
                    },
                    three_point_count: PolicyValue {
                        value: 3,
                        rationale: "Scallop quality sweep shape: tighter, baseline, looser.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Three quality-target probes are enough for first-pass optimization.",
                        },
                    },
                    four_point_count: PolicyValue {
                        value: 3,
                        rationale: "Scallop height has no four-point mode today; value kept explicit so future changes are reviewed.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "No current operation needs four scallop-height probes.",
                        },
                    },
                    midpoint_weight: PolicyValue {
                        value: 0.5,
                        rationale: "Unused for today's three-point scallop sweep; kept explicit for any future four-point quality sweep.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "If scallop needs a fourth point, midpoint should mirror DOC/stepover semantics.",
                        },
                    },
                    hard_floor: PolicyValue {
                        value: 0.01,
                        rationale: "Sub-10um scallop targets behave like polishing passes and can explode toolpath length with little wood-surface gain.",
                        source: PolicySource::PhysicalLimit,
                    },
                    hard_ceiling: PolicyValue {
                        value: 10.0,
                        rationale: "Scallop heights above 10mm describe a finish operation that's effectively roughing; well above operator quality intent.",
                        source: PolicySource::PhysicalLimit,
                    },
                    dedup_tolerance: PolicyValue {
                        value: 0.001,
                        rationale: "1um tolerance is appropriate because scallop-height values are an order of magnitude smaller than stepover values.",
                        source: PolicySource::Derived {
                            from: "scallop-height axis scale",
                        },
                    },
                    allow_outside_preferred: PolicyValue {
                        value: true,
                        rationale: "No LUT-preferred envelope currently exists for scallop height; local quality probes remain allowed.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Operator quality intent is preserved by staying near baseline.",
                        },
                    },
                    outside_preferred_hi_mult: PolicyValue {
                        value: 1.15,
                        rationale: "Reserved — scallop has no LUT-preferred envelope today, so probes never fire.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Symmetric to DOC/stepover for future use.",
                        },
                    },
                    outside_preferred_lo_mult: PolicyValue {
                        value: 0.85,
                        rationale: "Reserved — scallop has no LUT-preferred envelope today, so probes never fire.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "Symmetric to DOC/stepover for future use.",
                        },
                    },
                },
            },
            feed: FeedPolicy {
                min_positive_scale_input: PolicyValue {
                    value: 1.0,
                    rationale: "Never let optimizer math use zero or negative feed/RPM scale denominators.",
                    source: PolicySource::PhysicalLimit,
                },
                dedup_threshold_fraction: PolicyValue {
                    value: 0.01,
                    rationale: "Feed changes below 1% are noise-level relative to controller and sim uncertainty.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "Skipping <1% retargets avoids wasting sims on indistinguishable candidates.",
                    },
                },
                plunge_tracking_threshold_fraction: PolicyValue {
                    value: 0.10,
                    rationale: "Plunge tracks feed only for meaningful feed changes to avoid churn on small retargets.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "10% separates real feeds/speeds changes from numeric noise.",
                    },
                },
                delta_display_tolerance_mm_min: PolicyValue {
                    value: 0.5,
                    rationale: "Do not show sub-0.5 mm/min feed drift as a user-visible parameter change.",
                    source: PolicySource::Derived {
                        from: "GUI/display precision",
                    },
                },
                scale_floor: PolicyValue {
                    value: 1.0,
                    rationale: "Stage 0 headroom never recommends scaling down; downscaling belongs to retarget/grid stages.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "Headroom stage should only surface faster candidates.",
                    },
                },
                scale_epsilon: PolicyValue {
                    value: 1e-6,
                    rationale: "Floating-point epsilon for distinguishing no-headroom scale factors from exactly 1x.",
                    source: PolicySource::Derived {
                        from: "f64 arithmetic tolerance",
                    },
                },
                lut_rpm_nominal_headroom: PolicyValue {
                    value: 1.2,
                    rationale: "When LUT only has nominal RPM, permit the same +20% headroom as the existing ED5 bracket.",
                    source: PolicySource::Handbook {
                        citation: "Engineering Default 5, optimizer redesign 2026-05-08",
                    },
                },
            },
            retarget: RetargetPolicy {
                chipload_target_midpoint_weight: PolicyValue {
                    value: 0.5,
                    rationale: "With both chipload bounds present, target the midpoint of the vendor envelope.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "Mid-envelope leaves symmetric margin to burn and breakage limits.",
                    },
                },
                rpm_bracket_midpoint_weight: PolicyValue {
                    value: 0.5,
                    rationale: "When LUT RPM min and max are both present, target their midpoint for retargeting.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "Mid-bracket RPM leaves symmetric margin to row limits.",
                    },
                },
                chipload_upper_only_fraction: PolicyValue {
                    value: 0.85,
                    rationale: "Rows with only chipload max target 85% of max to leave breakage margin.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "15% margin is enough without being overly conservative.",
                    },
                },
                chipload_lower_only_headroom: PolicyValue {
                    value: 1.15,
                    rationale: "Rows with only chipload min target 15% above the burn/rubbing floor.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "15% margin clears burn risk without jumping too far on sparse data.",
                    },
                },
                chipload_low_headroom: PolicyValue {
                    value: 1.2,
                    rationale: "Future sample-driven BurnRisk retarget should aim 20% above LUT minimum.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "20% margin avoids immediately re-triggering low-chipload noise.",
                    },
                },
                chipload_high_headroom: PolicyValue {
                    value: 1.2,
                    rationale: "Future BreakageRisk retarget should aim 20% below LUT maximum.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "20% margin avoids immediately re-triggering high-chipload spikes.",
                    },
                },
                power_headroom: PolicyValue {
                    value: 0.85,
                    rationale: "Future power retarget should leave 15% spindle-power margin.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "15% margin covers router power and material variability.",
                    },
                },
                deflection_headroom: PolicyValue {
                    value: 0.75,
                    rationale: "Future deflection retarget should aim below the threshold to leave stiffness margin.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "25% margin avoids oscillating around the deflection limit.",
                    },
                },
                extrapolation_approx_ln_threshold: PolicyValue {
                    value: 0.336,
                    rationale: "ln(1.4); LUT scaling beyond roughly +/-40% should be marked approximate.",
                    source: PolicySource::Handbook {
                        citation: "G5/G6/G7 vendor-LUT extrapolation policy, 2026-05-08",
                    },
                },
            },
            ranking: RankingPolicy {
                recommendation_cycle_delta_s: PolicyValue {
                    value: 0.5,
                    rationale: "Minimum cycle-time improvement worth surfacing as a recommendation.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "0.5s is above sim/float noise and below user-perceptible cycle-time changes.",
                    },
                },
                failing_gate_relative_threshold: PolicyValue {
                    value: 0.05,
                    rationale: "Both-failing gate deltas need a 5% relative improvement/worsening before classification changes.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "5% avoids flipping Improved/Worsened on noisy sim samples.",
                    },
                },
                failing_gate_absolute_epsilon: PolicyValue {
                    value: 1e-6,
                    rationale: "Absolute floor for gate-delta comparisons when baseline peak is near zero.",
                    source: PolicySource::Derived {
                        from: "f64 arithmetic tolerance",
                    },
                },
            },
            stages: StagePolicy {
                coarse_resolution_mm: PolicyValue {
                    value: 1.0,
                    rationale: "Coarse Stage-1 dexel resolution calibrated to keep verdict kinds stable while reducing sim cost.",
                    source: PolicySource::Handbook {
                        citation: "Engineering Default 1, wanaka calibration 2026-05-03",
                    },
                },
                refined_resolution_mm: PolicyValue {
                    value: 0.5,
                    rationale: "Default refined dexel resolution used for reported candidate verdicts and cycle times.",
                    source: PolicySource::Handbook {
                        citation: "Engineering Default 1, optimizer redesign 2026-05-08",
                    },
                },
                refined_survivor_count: PolicyValue {
                    value: 3,
                    rationale: "Stage 2 re-simulates top three coarse candidates by cycle time.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "Top three captures the useful frontier without excessive full-resolution sims.",
                    },
                },
            },
            fallback: FallbackPolicy {
                doc_anchor_mm: PolicyValue {
                    value: 1.5,
                    rationale: "Fallback DOC anchor when an op lacks depth_per_pass; should be replaced by explicit axis topology in Step 3.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "1.5mm matches conservative wood-router defaults for legacy grid plumbing.",
                    },
                },
                stepover_anchor_mm: PolicyValue {
                    value: 1.0,
                    rationale: "Fallback stepover anchor when an op lacks stepover; should be replaced by explicit axis topology in Step 3.",
                    source: PolicySource::TuningChoice {
                        hypothesis: "1.0mm is a neutral dummy value for collapsed no-op axes.",
                    },
                },
                scallop_height_anchor_mm: PolicyValue {
                    value: 0.1,
                    rationale: "Fallback scallop-height anchor matching ScallopConfig default.",
                    source: PolicySource::Handbook {
                        citation: "ScallopConfig default, G2 optimizer gap closure 2026-05-08",
                    },
                },
            },
            deflection_setup_target_um: PolicyValue {
                value: 50.0,
                rationale: "Within-bound deflection target used to estimate required stickout reduction in setup prescriptions.",
                source: PolicySource::Derived {
                    from: "deflection::WITHIN_BOUND_MM x 1000",
                },
            },
            bottleneck_fraction: PolicyValue {
                value: 0.30,
                rationale: "Project rollup flags a toolpath as bottleneck when it consumes at least 30% of total baseline cycle time.",
                source: PolicySource::TuningChoice {
                    hypothesis: "Wanaka TP6 at 61% should trip; small project-curve ops near 5% should not.",
                },
            },
        }
    }
}
