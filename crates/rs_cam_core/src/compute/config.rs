use serde::{Deserialize, Serialize};

/// Where the toolpath's stock material comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StockSource {
    /// Start from raw stock (default).
    #[default]
    Fresh,
    /// Simulate all prior enabled toolpaths to determine starting material.
    FromRemainingStock,
}

// Canonical definition now lives in `crate::ids` (R3 — id/index split);
// re-exported here so the long-standing `compute::config::ToolpathId`
// path keeps working.
pub use crate::ids::ToolpathId;

/// Why a toolpath cannot generate *yet* — and which operation it is waiting
/// for. A/M11.
///
/// This is a **sequencing** state, not a failure. An op with
/// `StockSource::FromRemainingStock` needs the simulated stock of the ops
/// before it, and that snapshot only exists after a *simulation*. So a rest op
/// can never see stock produced by an op generated earlier in the same
/// `generate_all` pass — measured on wanaka 2026-07-30, where `Rivers`
/// generated fine and its own successor `Lakes` failed in the same pass.
/// Reporting that as `Error` made "cannot yet" indistinguishable from "cannot
/// ever", and left the operator with no way to tell a one-round wait from a
/// four-round one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwaitingPriorStock {
    /// The upstream operation whose simulated stock is missing. `None` only
    /// when this op is first in its setup and there is genuinely nothing
    /// upstream to name — a real configuration error, spelled out in
    /// `message`.
    pub blocking_toolpath_id: Option<ToolpathId>,
    /// That operation's 0-based index at the time the block was recorded.
    /// Advisory: indices shift when toolpaths are added or removed, so
    /// resolve by `blocking_toolpath_id` when it matters.
    pub blocking_toolpath_index: Option<usize>,
    /// The operator-facing sentence. Always names the blocker when there is
    /// one. Shape pinned by a sentry so it cannot silently regress to the
    /// pre-A/M11 text, which named no operation and did not say the cycle may
    /// need repeating.
    pub message: String,
}

/// Where a toolpath stands. Consumed identically by the GUI badge, the MCP
/// `list_toolpaths` / diagnostics surface, and the generate_all reporter —
/// there is deliberately no second taxonomy.
#[derive(Debug, Clone)]
pub enum ComputeStatus {
    Pending,
    Computing,
    Done,
    /// A/M11 — blocked on sequencing, not failed. See [`AwaitingPriorStock`].
    AwaitingPriorStock(AwaitingPriorStock),
    /// The operation is switched off (`enabled: false`).
    ///
    /// **Never stored.** It is produced only by [`ComputeStatus::effective`],
    /// which lets `enabled: false` win over whatever the op last recorded.
    /// Storing it would mean deciding what to restore on re-enable; deriving
    /// it means a disabled op can never carry a live-looking error — the
    /// A/M11 defect where `3D Finish 6` read as broken when it was merely off.
    Disabled,
    /// A genuine generation failure. Something is wrong with the operation,
    /// its inputs, or its geometry; no amount of re-running will fix it.
    Error(String),
}

impl ComputeStatus {
    /// The status a reader should show, given whether the op is enabled.
    /// Every surface goes through here so GUI, MCP and CLI cannot drift.
    pub fn effective(enabled: bool, raw: &Self) -> &Self {
        static DISABLED: ComputeStatus = ComputeStatus::Disabled;
        if enabled { raw } else { &DISABLED }
    }

    /// Stable machine-readable label. Used by the MCP `status` field and the
    /// GUI tooltips.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Computing => "Computing",
            Self::Done => "Done",
            Self::AwaitingPriorStock(_) => "AwaitingPriorStock",
            Self::Disabled => "Disabled",
            Self::Error(_) => "Error",
        }
    }

    /// The failure text — and ONLY for genuine failures. A blocked op is not
    /// an error and must not inflate an error list; read it through
    /// [`Self::blocked_on`].
    pub fn error_text(&self) -> Option<&str> {
        match self {
            Self::Error(e) => Some(e.as_str()),
            Self::Pending
            | Self::Computing
            | Self::Done
            | Self::AwaitingPriorStock(_)
            | Self::Disabled => None,
        }
    }

    /// The sequencing block, when there is one.
    pub fn blocked_on(&self) -> Option<&AwaitingPriorStock> {
        match self {
            Self::AwaitingPriorStock(b) => Some(b),
            Self::Pending | Self::Computing | Self::Done | Self::Disabled | Self::Error(_) => None,
        }
    }

    /// Any message worth showing the operator, failure or block.
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::Error(e) => Some(e.as_str()),
            Self::AwaitingPriorStock(b) => Some(b.message.as_str()),
            Self::Pending | Self::Computing | Self::Done | Self::Disabled => None,
        }
    }

    /// `true` when this op still needs a generate to reach `Done`. A blocked
    /// op counts — it will generate once its upstream stock exists.
    pub fn needs_generation(&self) -> bool {
        match self {
            Self::Pending | Self::AwaitingPriorStock(_) => true,
            Self::Computing | Self::Done | Self::Disabled | Self::Error(_) => false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ToolpathStats {
    pub move_count: usize,
    pub cutting_distance: f64,
    pub rapid_distance: f64,
    /// Generation-time finding, not a toolpath measurement: region-interior
    /// area a scallop ring cascade left UNCUT because it hit its ring cap
    /// before collapsing — the **truncated cascade core**.
    ///
    /// **Renamed in wave 16** (Checkpoint E ruling A6). It was
    /// `standing_material_mm2` from A/M9 until 2026-08-04, and that name was
    /// wrong in the one way a measurement name must never be: §5.3 gave the
    /// cascade the M4 oracle's vocabulary, and *standing* there means
    /// "reached, left high" while this field measures ground **no cutter
    /// position ever entered**. That is the oracle's *untouched*. The
    /// reached-but-uncut quantity is [`Self::reached_uncut_estimate_mm2`],
    /// a different number entirely. `truncated_core_mm2` names the geometry
    /// it actually sums and claims nothing about why.
    ///
    /// The old spelling survives ONLY as a legacy JSON key emitted beside
    /// the new one on the two wires that carry it
    /// ([`crate::session::ToolpathDiagnostic`]'s `Serialize` and the CLI's
    /// per-toolpath report). No Rust identifier in this repo carries it.
    ///
    /// A consumer reading the CURRENT wire takes `truncated_core_mm2`
    /// plainly; `#[serde(alias = "standing_material_mm2")]` is for documents
    /// written BEFORE the rename, and must not be combined with reading the
    /// dual-key wire — serde rejects the same field arriving twice. Both
    /// halves of that are pinned in
    /// `tests/standing_material_channel_am9.rs`.
    ///
    /// **Three-valued on purpose** (A/M9, `MEASUREMENT_DOMAINS.md` X-19):
    ///
    /// * `None` — **not measured**. The operation runs no ring cascade
    ///   (any 2.5D family, drop-cutter, waterline, drill …), or the result
    ///   came from a path that carries no [`GenerationFindings`]. Do NOT
    ///   read this as "nothing left uncut"; it supports no ratio at all.
    /// * `Some(0.0)` — **measured zero**: a cascade ran and collapsed, so
    ///   no core was left truncated.
    /// * `Some(a)` — `a` mm² of region interior was never reached.
    ///
    /// The pre-A/M9 `f64` conflated the first two, which is the silent-zero
    /// trap the audit logged: a reader building "% left uncut" over a
    /// pocket would have divided a real area by an unmeasured zero.
    ///
    /// Domain / stage / resolution are fixed and declared by
    /// [`TRUNCATED_CORE_PROVENANCE`], from which
    /// [`TRUNCATED_CORE_DOMAIN`], [`TRUNCATED_CORE_STAGE`] and
    /// [`TRUNCATED_CORE_RESOLUTION`] are derived — every user-visible
    /// rendering of this number must state them (M1). Read the typed value
    /// through [`ToolpathStats::truncated_core`], which returns the area
    /// and its provenance together or `None`. Sourced from
    /// [`crate::compute::execute::GenerationFindings`]; read by the
    /// diagnostics pipeline as `crate::diagnostics::ids::GEOM_STANDING_MATERIAL`
    /// — the diagnostic **id** is a stable identity and deliberately keeps
    /// the old word (`MEASUREMENT_DOMAINS.md` renaming table).
    ///
    /// Report-only: no gate consumes it and no verdict changes on it.
    pub truncated_core_mm2: Option<f64>,
    /// M4 §5b: the HOLE-AWARE sibling of [`Self::truncated_core_mm2`].
    /// `truncated_core_mm2` is computed from each truncated cascade
    /// polygon's EXTERIOR only (`MEASUREMENT_DOMAINS.md` X-5) — an island
    /// inside the truncated core over-reports as uncut. This field nets out
    /// holes instead, straight off
    /// [`crate::scallop::ScallopReport::untouched_mm2`].
    ///
    /// Same three-valued contract as [`Self::truncated_core_mm2`]: `None`
    /// = not measured, `Some(0.0)` = measured and clean.
    /// `untouched_material_mm2 <= truncated_core_mm2` whenever both are
    /// `Some` (same cascade run, hole-corrected). Read through
    /// [`ToolpathStats::untouched_material`].
    ///
    /// Report-only: no gate consumes it and no verdict changes on it.
    pub untouched_material_mm2: Option<f64>,
    /// M4 §5b: area the ring cascade **DID reach** — it ringed there — but
    /// where the keep predicate dropped every point, so no cut landed.
    /// Straight off [`crate::scallop::ScallopReport::standing_mm2`]. Same
    /// three-valued contract.
    ///
    /// **This is NOT an estimate of [`Self::truncated_core_mm2`].** It is
    /// a different quantity, and the naming here is a trap worth stating
    /// plainly (H4, wave 15 — this field was called
    /// `standing_material_estimate_mm2` for exactly long enough to prove the
    /// point):
    ///
    /// | field | what it is | oracle's word |
    /// |---|---|---|
    /// | [`Self::truncated_core_mm2`] | truncated cascade core, exteriors only | **untouched** (never reached) |
    /// | [`Self::untouched_material_mm2`] | the same core, hole-corrected | **untouched** (never reached) |
    /// | this field | ringed, then every point dropped | **standing** (reached, left high) |
    ///
    /// Wave 15 shipped that table under a first column that still read
    /// `standing_material_mm2` — a field labelled *standing* sitting in the
    /// *untouched* row. Checkpoint E ruling A6 closed it: the first row's
    /// field is now [`Self::truncated_core_mm2`], and only this field wears
    /// the oracle's word *standing*.
    ///
    /// **Not an exact area** — it is `(arc length owned by dropped ring
    /// points) x (offset stepover)`, summed per ring; see the source field's
    /// doc for what it cannot distinguish (off-part geometry vs a genuine
    /// left-high residual). Read through
    /// [`ToolpathStats::reached_uncut_estimate`].
    ///
    /// Report-only: no gate consumes it and no verdict changes on it.
    pub reached_uncut_estimate_mm2: Option<f64>,
    /// Wave D1: a planned finish BAND whose cutting was entirely erased by
    /// height resolution — an unmachined feature.
    ///
    /// Three-valued for the same reason as [`Self::truncated_core_mm2`]:
    /// `None` = **not measured** (this operation plans no bands at all —
    /// anything that is not a `UnifiedFinish`), `Some(f)` = a banded
    /// decomposition ran AND at least one band was dropped. A banded op that
    /// dropped nothing also reports `None`, because "no dropped band" and
    /// "nothing to drop" are the same statement about the part: the honest
    /// distinction narration draws is *measured-clean* vs *not measured*, and
    /// it draws it from the operation kind, not from this field.
    ///
    /// Report-only: generation still succeeds, the diagnostic severity is
    /// `Caution`, and no verdict reads it.
    ///
    /// **Boxed on purpose.** The finding carries a whole
    /// [`crate::measurement::MeasurementProvenance`] and is `None` on almost
    /// every toolpath, so paying 8 bytes here instead of ~120 keeps
    /// `ToolpathStats` — cloned once per toolpath into the session results
    /// and again into GUI state — small, without weakening the measurement
    /// contract.
    ///
    /// This used to be justified by `clippy::large_enum_variant` firing on
    /// the GUI's `ComputeMessage` channel enum. That is no longer the
    /// reason: C5 boxed `ComputeMessage::Toolpath` itself, so nothing added
    /// here can push that enum over the threshold again.
    pub dropped_band: Option<Box<DroppedBandFinding>>,
    /// Wave D1: the tip-float residual on a pencil/rest centreline — points
    /// where the cutter physically cannot reach the valley floor it is being
    /// driven along, and the depth it floats above it.
    ///
    /// `None` = **not measured**: the operation emits no valley centrelines
    /// (Checkpoint A evidence §9.4). `Some` with `floating_points == 0` is a
    /// measured clean pass. Never read a missing value as zero float.
    ///
    /// Report-only: no gate consumes it.
    pub tip_float: Option<TipFloatFinding>,
    /// PR-5: a loaded project carries a RETIRED dial at a non-default value,
    /// so the number the operator set is no longer the one steering the
    /// operation.
    ///
    /// `None` = **nothing retired is set** (the overwhelming majority of
    /// toolpaths, and every project that never touched the dial). It is not
    /// a measurement, it is a compatibility notice: the field is still
    /// deserialized so old projects load unchanged, and this is what stops
    /// that from being silent.
    ///
    /// **Boxed** for the same reason as [`Self::dropped_band`]: keeping
    /// `ToolpathStats` small for the clones it takes per toolpath. Not,
    /// since C5, because of `clippy::large_enum_variant`.
    ///
    /// Report-only: no gate consumes it, and generation is unaffected.
    pub deprecated_dial: Option<Box<DeprecatedDialFinding>>,
    /// PR-6a (H2.3): an offset stepover this operation DERIVED from the
    /// canonical reach policy instead of taking from a dial, together with
    /// the envelope-scaled number that used to be used there.
    ///
    /// EMPTY = **this operation derived no stepover** (anything that is not
    /// a `UnifiedFinish` running its crease/pencil claims pipeline, and not
    /// a rest-analysis post-pass that sized its own). These are not
    /// measurements of the part and not defect claims; they are the audit
    /// trail for numbers the operator cannot see in any dial.
    ///
    /// C8: a `Vec`, not `Option<Box<..>>`. Two derivations can occur on one
    /// toolpath — the operation's own routing site and PR-7's generic
    /// rest-analysis post-pass — and the slot kept only the first. Reading
    /// ergonomics improve rather than degrade: `iter().filter(..)` replaces
    /// `as_deref().map(..)`, and "did anything derive a stepover" is
    /// `!is_empty()`.
    ///
    /// The `Box` is gone with the `Option`: the finding is 6 words, and a
    /// `Vec` is 3 whether or not it allocates. Empty is the common case and
    /// allocates nothing.
    ///
    /// Report-only: no gate consumes it.
    pub derived_stepovers: Vec<DerivedStepoverFinding>,
    /// C8: a planned finish band whose Z ladder the resolved heights
    /// SHORTENED while it still cut. `None` = **no band was partially
    /// clipped**, or nothing that plans bands ran — never "measured zero".
    /// Its loud sibling is [`Self::dropped_band`]; the two are disjoint by
    /// construction.
    ///
    /// **Boxed** for the same reason as [`Self::dropped_band`]: keeping
    /// `ToolpathStats` small for the clones it takes per toolpath.
    ///
    /// Report-only: no gate consumes it.
    pub clipped_band: Option<Box<ClippedBandFinding>>,
    /// PR-8b (H3): how far a ramp-finish descent had to be RAISED because the
    /// cutter could not hold the commanded depth there.
    ///
    /// `None` = **no ramp descent ran**, so nothing was measured. `Some` with
    /// `clamped_points == 0` and an unmoved ladder bottom is the honest
    /// "a ramp ran and every commanded depth was holdable" — the A/M9
    /// distinction, applied to a second measure (X-19).
    ///
    /// **Boxed** for the same reason as [`Self::dropped_band`]: keeping
    /// `ToolpathStats` small for the clones it takes per toolpath. Not,
    /// since C5, because of `clippy::large_enum_variant`.
    ///
    /// Report-only: no gate consumes it. The clamp itself is not report-only
    /// — it changes emitted geometry — but nothing downstream branches on
    /// this record.
    pub ramp_reach_clamp: Option<Box<crate::ramp_finish::RampReachClamp>>,
    /// A/M6: which rest reference this operation's crease/pencil claims
    /// pipeline ran against, and whether that was pinned or derived.
    ///
    /// `None` = **the claims pipeline did not run**, so no reference was
    /// resolved. That is every operation except a `UnifiedFinish` with
    /// `pencil_claims = true` — including a `UnifiedFinish` with the dial at
    /// its default, where `claims_reference` is inert and reporting it would
    /// be reporting a decision nothing acted on.
    ///
    /// NOT boxed, unlike its neighbours: the payload is one fieldless enum
    /// plus a `bool`, so a `Box` would cost a pointer to save nothing.
    ///
    /// Report-only: no gate consumes it. The *resolution* is not report-only
    /// — it decides which field the detector reads — but nothing downstream
    /// branches on this record.
    pub claims_reference: Option<ClaimsReferenceFinding>,
    /// A/M7 gate 1: retract round-trip count, split by in-routing-node vs
    /// between-nodes. See [`RetractTripCount`] for the counting rule and
    /// the X-19 availability contract of the in/out split.
    ///
    /// `None` = **this stats struct never walked a move list**
    /// (`ToolpathStats::default()` placeholders, and results built before a
    /// real generation ran) — never "zero trips". Every stats struct that
    /// went through [`crate::compute::stats::compute_stats_with_spans`] or
    /// the production `generate_toolpath` literal carries `Some`, even on a
    /// toolpath with zero rapids (`total == 0` is a measured answer, not an
    /// absent one).
    ///
    /// Report-only: no gate consumes it.
    pub retract_trips: Option<RetractTripCount>,
}

/// A/M7 gate 1: how many retract round trips a toolpath took, and whether
/// each one happened INSIDE a planner routing node or BETWEEN two of them.
///
/// The reason this channel exists: finishing air cost is COUNT-bound, not
/// distance-bound — a hop pays two ~`safe_z` Z legs whatever its XY length
/// — and `ToolpathStats::rapid_distance` cannot show that. The v3 process-
/// proof campaign found 15 311 of 15 363 measured trips landing INSIDE a
/// single routing node, which is why the split has to travel with the
/// count rather than being left as a derivation nobody performs.
///
/// A "trip" is one maximal contiguous run of
/// [`crate::toolpath::MoveType::Rapid`] moves — the exact rule
/// `tests/v3_cascade_ab.rs`'s `rapid_round_trips` helper uses, reproduced
/// bit-for-bit in [`crate::compute::stats::compute_retract_trips`] so the
/// production channel and that harness can never disagree about what counts
/// as one trip.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RetractTripCount {
    /// Total retract round trips. Always populated once a stats struct has
    /// walked a move list — needs no spans, so this is never `None` inside
    /// a `Some(RetractTripCount)`.
    pub total: usize,
    /// Of `total`, how many trips STARTED inside a planner territory
    /// `Region` node (`RegionSpanRole::Node`). `None` when the split could
    /// not be trusted — no spans were supplied, or
    /// `AnnotatedToolpath::spans_valid` was `false` at compute time (X-19:
    /// an untrustworthy split must report as unmeasured, never as a
    /// confident zero).
    pub in_node: Option<usize>,
    /// Of `total`, how many trips started between two nodes (or outside
    /// any node). Same availability rule as [`Self::in_node`]. When both
    /// are `Some`, `in_node + between_nodes == total`.
    pub between_nodes: Option<usize>,
    /// Rapid distance (mm) attributable to the in-node trips. Same
    /// availability rule as [`Self::in_node`].
    pub in_node_rapid_mm: Option<f64>,
    /// Rapid distance (mm) attributable to the between-node trips. Same
    /// availability rule as [`Self::between_nodes`]. When both mm fields
    /// are `Some`, they sum to [`ToolpathStats::rapid_distance`] (up to
    /// floating-point accumulation order).
    pub between_nodes_rapid_mm: Option<f64>,
}

impl RetractTripCount {
    /// `true` when the in-node / between-node split is present and can be
    /// trusted (spans were supplied and valid at compute time).
    #[must_use]
    pub const fn has_split(&self) -> bool {
        self.in_node.is_some() && self.between_nodes.is_some()
    }
}

/// Which rest reference a claims pipeline resolved to, and under what
/// conditions (A/M6).
///
/// The finding exists because the resolution is invisible everywhere else:
/// `claims_reference` is a three-valued dial whose `auto` setting means
/// "decide from context", and the context — whether a simulated prior stock
/// is in scope — is not a field of any config. Before A/M6 the only trace of
/// the decision was a `tracing::warn!` on one of the six outcomes, in a
/// process that usually installs no subscriber.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimsReferenceFinding {
    /// What the dial said, what was used, and whether a machined prior was
    /// in scope — see [`crate::unified_finish::ClaimsReferenceResolution`].
    pub resolution: crate::unified_finish::ClaimsReferenceResolution,
    /// The operation also asked for S4 rest-territory confinement
    /// (`territory_clip`), which only runs under a machined-stock reference.
    /// When this is `true` and the resolution is a self-probe one, the
    /// confinement was SKIPPED — the difference between a rest pass and an
    /// all-over pass.
    pub territory_clip_requested: bool,
}

impl ClaimsReferenceFinding {
    /// `true` when the operator asked for rest-territory confinement and the
    /// resolved reference cannot deliver it, so the operation quietly became
    /// an all-over pass.
    #[must_use]
    pub const fn territory_clip_skipped(&self) -> bool {
        self.territory_clip_requested
            && matches!(
                self.resolution.reference(),
                crate::unified_finish::CreaseReference::SelfProbe
            )
    }
}

/// A user-facing dial that a project still sets but the code no longer
/// reads (PR-5, H2.2).
///
/// The alternative to reporting is one of the two failure modes this
/// programme keeps finding: silently ignore the value (the operator's
/// setting stops doing anything, with no way to tell), or refuse to load
/// the project (breaks every saved job for a dial that was never
/// load-bearing). Deserialize it, ignore it, and SAY SO.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeprecatedDialFinding {
    /// The dial's name as it appears in the project file and the GUI.
    pub dial: &'static str,
    /// The value the project carries.
    pub value: f64,
    /// The value that used to be the default — anything else means the
    /// operator deliberately tuned it.
    pub default_value: f64,
    /// One sentence naming what replaced it.
    pub replaced_by: &'static str,
}

/// An offset stepover an operation sized from [`crate::reach`] rather than
/// from a user dial (PR-6a, H2.3).
///
/// The number this records is invisible to the operator: it is not a field
/// in any config, it steers both the routing criterion and the emitted fan,
/// and until PR-6a it was `cutter.envelope_radius_mm() * 0.5` — half the
/// SHANK of a tapered ball, three times the whole tip. Reporting the derived
/// value AND the retired one is what makes the migration checkable on a real
/// job instead of only on a fixture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DerivedStepoverFinding {
    /// Which routing/fit site this sized, e.g.
    /// `"UnifiedFinish crease/pencil claims"`.
    pub site: &'static str,
    /// The stepover (mm) actually used, from
    /// [`crate::reach::suggested_offset_stepover_mm`].
    pub stepover_mm: f64,
    /// Depth (mm) the reach policy was evaluated at. The policy is
    /// depth-aware; a single scalar stepover has to name ITS depth.
    pub reference_depth_mm: f64,
    /// One phrase saying why that depth was the honest one to size at.
    pub reference_depth_basis: &'static str,
    /// What the retired `envelope_radius_mm() * 0.5` rule would have
    /// produced on this tool. Equal to `stepover_mm` on any plain ball.
    pub envelope_rule_mm: f64,
}

impl DerivedStepoverFinding {
    /// `true` when the policy value and the retired envelope rule coincide —
    /// every plain ball, at every depth. Nothing moved, so nothing is worth
    /// telling the operator.
    #[must_use]
    pub fn matches_the_envelope_rule(&self) -> bool {
        (self.stepover_mm - self.envelope_rule_mm).abs() <= 1e-9
    }
}

/// A finish band whose planned cutting was entirely removed by height
/// resolution (Wave D1, ledger task #15).
///
/// The mechanism this exists to make audible: `UnifiedFinishConfig`'s
/// `depth_semantics()` is `DepthSemantics::None`, so an Auto `bottom_z`
/// resolves to `top_z - 0.0` — the stock top — and the very-steep band's
/// waterline ladder (`final_z = band_min_z.max(bottom_z)`) collapses onto
/// the rim. The band planned real levels over real area and emitted no
/// cutting at all; before this finding the only trace was a
/// `region_count += 1` on a struct nothing printed.
///
/// Deliberately NOT the fix. Changing the depth semantics is a behavioural
/// change; this is the instrument that must exist first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DroppedBandFinding {
    /// Stable band token (`"VerySteep"`, `"MidSteep"`, `"Shallow"`) —
    /// `crate::unified_finish::RegionKind::band_label`. Names the band of the
    /// LARGEST dropped region when several were dropped.
    pub band_label: &'static str,
    /// How many planned regions were dropped, across every band.
    pub region_count: usize,
    /// Summed XY-projected area (mm²) of the dropped regions' band polygons.
    /// Read the domain off [`Self::provenance`] before comparing it to
    /// anything.
    pub area_mm2: f64,
    /// The resolved height (mm, operation frame) that clipped the largest
    /// dropped region.
    pub clip_z_mm: f64,
    /// Which resolved height it was: `"bottom_z"` or `"top_z"`.
    pub clip_label: &'static str,
    /// What [`Self::area_mm2`] means. Carried per-instance rather than as a
    /// module constant because the band polygons are quantised by the
    /// classification grid, whose cell size is derived from the TOOL
    /// (`cusp_radius/4`) — one constant could not describe two tools.
    pub provenance: crate::measurement::MeasurementProvenance,
}

impl DroppedBandFinding {
    /// [`Self::area_mm2`] in the newtype that refuses cross-domain division
    /// (M1 slice 2), together with its contract.
    #[must_use]
    pub fn area(
        &self,
    ) -> (
        crate::measurement::ProjectedXyAreaMm2,
        crate::measurement::MeasurementProvenance,
    ) {
        (
            crate::measurement::ProjectedXyAreaMm2::new(self.area_mm2),
            self.provenance,
        )
    }
}

/// A planned finish band whose Z ladder was SHORTENED by the resolved
/// heights but which still emitted cutting (C8).
///
/// The complement of [`DroppedBandFinding`], and the gap
/// `ANTIPATTERNS_BACKLOG.md` P8 logged. Wave D1 measured the clip on EVERY
/// band and discarded the measurement unless the region emitted nothing at
/// all — so a band that machined the top 2 mm of a 12 mm wall and stopped
/// left an unfinished feature and no trace.
///
/// Severity is the reason the two are separate types rather than one with a
/// flag: a dropped band is a `Caution` ("this feature will be UNMACHINED"),
/// a clipped band is `Info` ("this feature is PARTLY machined, here is what
/// was left"), and a loud finding must not be buried under quiet ones.
///
/// Report-only: no gate consumes it, and it is deliberately NOT the fix.
/// Changing `UnifiedFinishConfig`'s depth semantics is a behavioural change;
/// this is the instrument that has to exist first.
///
/// **Known limit, stated rather than implied**: only the `VerySteep` arm
/// measures its clip today. The `MidSteep` (scallop) and `Shallow`
/// (drop-cutter raster) arms never set one, so a partial clip there is
/// still invisible — Wave D1 built the instrument on the arm whose ladder is
/// explicit, and C8 widened what that instrument REPORTS without widening
/// where it is taken.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClippedBandFinding {
    /// Stable band token — names the band of the LARGEST clipped region.
    pub band_label: &'static str,
    /// How many planned regions were clipped, across every band.
    pub region_count: usize,
    /// Summed XY-projected area (mm²) of the clipped regions' band polygons.
    /// Read the domain off [`Self::provenance`] before comparing it.
    pub area_mm2: f64,
    /// The resolved height (mm) that clipped the largest region.
    pub clip_z_mm: f64,
    /// Which resolved height it was: `"bottom_z"` or `"top_z"`.
    pub clip_label: &'static str,
    /// Ceiling (mm) the largest clipped band's own surface span asked for.
    pub requested_top_z_mm: f64,
    /// Floor (mm) it asked for.
    pub requested_bottom_z_mm: f64,
    /// Ceiling (mm) the resolved heights allowed.
    pub delivered_top_z_mm: f64,
    /// Floor (mm) they allowed.
    pub delivered_bottom_z_mm: f64,
    /// Z levels the largest clipped band would have laddered.
    pub planned_levels: usize,
    /// Z levels that survived.
    pub resolved_levels: usize,
    /// Worst vertical extent (mm) removed from any one clipped band —
    /// requested height minus delivered height. This is the number that
    /// answers "how much of the wall is unfinished".
    pub max_lost_height_mm: f64,
    /// What [`Self::area_mm2`] means. Per-instance for the same reason as
    /// [`DroppedBandFinding::provenance`]: the band polygons are quantised
    /// by a TOOL-derived classification cell.
    pub provenance: crate::measurement::MeasurementProvenance,
}

impl ClippedBandFinding {
    /// [`Self::area_mm2`] in the newtype that refuses cross-domain division
    /// (M1 slice 2), together with its contract.
    #[must_use]
    pub fn area(
        &self,
    ) -> (
        crate::measurement::ProjectedXyAreaMm2,
        crate::measurement::MeasurementProvenance,
    ) {
        (
            crate::measurement::ProjectedXyAreaMm2::new(self.area_mm2),
            self.provenance,
        )
    }
}

/// Tip float on a pencil/rest centreline: the cutter is driven along a
/// valley it physically cannot bottom out in, so it rides the walls and
/// leaves residual material below the emitted line (Wave D1; Checkpoint A
/// evidence §5 measured up to 5.248 mm on 24 of 176 taper cells, with no
/// channel of any kind reporting it).
///
/// The measurement is the one routing already solves: at each emitted
/// centreline point, the drop-cutter's resting Z minus the valley-floor Z
/// the detector traced at the same XY — the same quantity
/// `pencil::reach_gap_at_point` computes for the rest-depth gate, sampled
/// on every point instead of eight.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TipFloatFinding {
    /// Emitted centreline points examined (offset passes excluded — they are
    /// *meant* to ride the walls).
    pub centreline_points: usize,
    /// Of those, how many float more than [`TIP_FLOAT_THRESHOLD_MM`] above
    /// the traced valley floor.
    pub floating_points: usize,
    /// Largest float residual (mm) seen. `0.0` when nothing floated.
    pub max_float_mm: f64,
}

impl TipFloatFinding {
    /// Record one centreline point's float.
    ///
    /// NaN floats (a point whose lift found no contact, or a detector line
    /// with no surface Z) count as examined and nothing else — an unmeasured
    /// point is not a clean one, and it is certainly not a defect claim.
    pub fn record(&mut self, float_mm: f64) {
        self.centreline_points += 1;
        if float_mm > TIP_FLOAT_THRESHOLD_MM {
            self.floating_points += 1;
            if float_mm > self.max_float_mm {
                self.max_float_mm = float_mm;
            }
        }
    }

    /// Fold another tally in (one per chain / per detector arm).
    pub fn merge(&mut self, other: Self) {
        self.centreline_points += other.centreline_points;
        self.floating_points += other.floating_points;
        if other.max_float_mm > self.max_float_mm {
            self.max_float_mm = other.max_float_mm;
        }
    }

    /// Fraction of examined centreline points that float. `None` when
    /// nothing was examined — never a fabricated zero.
    #[must_use]
    pub fn floating_fraction(&self) -> Option<f64> {
        (self.centreline_points > 0)
            .then(|| self.floating_points as f64 / self.centreline_points as f64)
    }
}

/// Float (mm) above the traced valley floor at which a centreline point
/// counts as FLOATING.
///
/// Deliberately the same absolute number as
/// `crate::pencil::reach_gap_threshold` — 0.05 mm — because it is the same
/// physical question the pencil's own rest gate asks ("is the tool actually
/// off the surface here, or is this triangulation noise?"), and two
/// thresholds for one question is how instruments start disagreeing with
/// the code they measure.
pub const TIP_FLOAT_THRESHOLD_MM: f64 = 0.05;

/// The measurement contract of [`TipFloatFinding::max_float_mm`].
///
/// A vertical residual at a point, measured at generation from the
/// centreline drop solve. Not an area, not a length along the path, and not
/// comparable to either.
pub const TIP_FLOAT_PROVENANCE: crate::measurement::MeasurementProvenance =
    crate::measurement::MeasurementProvenance::new(
        crate::measurement::MeasurementDomain::VerticalResidualMm,
        crate::measurement::MeasurementStage::CentrelineDropSolve,
    )
    .with_resolution_note(
        "one sample per emitted centreline point (path sampling spacing); float = \
         drop-cutter rest Z minus the detector's traced valley-floor Z at the same XY",
    );

/// Measurement domain of [`TipFloatFinding::max_float_mm`].
pub const TIP_FLOAT_DOMAIN: &str = TIP_FLOAT_PROVENANCE.domain.label();

/// Pipeline stage [`TipFloatFinding`] is measured at.
pub const TIP_FLOAT_STAGE: &str = TIP_FLOAT_PROVENANCE.stage.label();

/// Resolution note of [`TipFloatFinding`].
pub const TIP_FLOAT_RESOLUTION: &str = TIP_FLOAT_PROVENANCE.resolution_note;

impl ToolpathStats {
    /// [`Self::truncated_core_mm2`] with its measurement contract attached,
    /// or `None` when nothing measured it (M1 slice 1).
    ///
    /// The provenance cannot be set independently of the value — the two
    /// travel together or not at all — so a stats struct can never claim a
    /// domain it did not measure in.
    #[must_use]
    pub fn truncated_core(
        &self,
    ) -> Option<(
        crate::measurement::ProjectedXyAreaMm2,
        crate::measurement::MeasurementProvenance,
    )> {
        self.truncated_core_mm2.map(|mm2| {
            (
                crate::measurement::ProjectedXyAreaMm2::new(mm2),
                TRUNCATED_CORE_PROVENANCE,
            )
        })
    }

    /// [`Self::untouched_material_mm2`] with its measurement contract
    /// attached, or `None` when nothing measured it. M4 §5b sibling of
    /// [`Self::truncated_core`] — same rule, different provenance
    /// ([`UNTOUCHED_MATERIAL_PROVENANCE`], hole-aware net area).
    #[must_use]
    pub fn untouched_material(
        &self,
    ) -> Option<(
        crate::measurement::ProjectedXyAreaMm2,
        crate::measurement::MeasurementProvenance,
    )> {
        self.untouched_material_mm2.map(|mm2| {
            (
                crate::measurement::ProjectedXyAreaMm2::new(mm2),
                UNTOUCHED_MATERIAL_PROVENANCE,
            )
        })
    }

    /// [`Self::reached_uncut_estimate_mm2`] with its measurement
    /// contract attached, or `None` when nothing measured it. M4 §5b
    /// sibling of [`Self::truncated_core`] — same rule, different
    /// provenance ([`REACHED_UNCUT_ESTIMATE_PROVENANCE`], an estimator,
    /// not an exact area).
    #[must_use]
    pub fn reached_uncut_estimate(
        &self,
    ) -> Option<(
        crate::measurement::ProjectedXyAreaMm2,
        crate::measurement::MeasurementProvenance,
    )> {
        self.reached_uncut_estimate_mm2.map(|mm2| {
            (
                crate::measurement::ProjectedXyAreaMm2::new(mm2),
                REACHED_UNCUT_ESTIMATE_PROVENANCE,
            )
        })
    }

    /// [`Self::tip_float`] with its measurement contract attached, or `None`
    /// when nothing measured it. Same rule as [`Self::truncated_core`]:
    /// the value and its provenance travel together or not at all.
    #[must_use]
    pub fn tip_float_measured(
        &self,
    ) -> Option<(TipFloatFinding, crate::measurement::MeasurementProvenance)> {
        self.tip_float.map(|f| (f, TIP_FLOAT_PROVENANCE))
    }

    /// [`Self::retract_trips`] with its measurement contract attached, or
    /// `None` when nothing measured it. Same rule as
    /// [`Self::truncated_core`]: the value and its provenance travel
    /// together or not at all.
    #[must_use]
    pub fn retract_trip_measurement(
        &self,
    ) -> Option<(RetractTripCount, crate::measurement::MeasurementProvenance)> {
        self.retract_trips.map(|f| (f, RETRACT_TRIP_PROVENANCE))
    }
}

/// The measurement contract of [`RetractTripCount`] — a count of retract
/// round trips, measured on the emitted toolpath with no reference to
/// stock (a rapid run is visible in the move list regardless of whether a
/// simulation ever runs).
pub const RETRACT_TRIP_PROVENANCE: crate::measurement::MeasurementProvenance =
    crate::measurement::MeasurementProvenance::new(
        crate::measurement::MeasurementDomain::RetractTripCount,
        crate::measurement::MeasurementStage::Emission,
    )
    .with_resolution_note(
        "one contiguous run of MoveType::Rapid moves = one retract round trip; the \
         in-node / between-nodes split classifies each run by whether its FIRST move \
         sits inside a planner territory Region node (RegionSpanRole::Node) — see \
         `AnnotatedToolpath::spans_valid` for when that split is trustworthy",
    );

/// Measurement domain of [`RetractTripCount::total`] and its split.
pub const RETRACT_TRIP_DOMAIN: &str = RETRACT_TRIP_PROVENANCE.domain.label();

/// Pipeline stage [`RetractTripCount`] is measured at.
pub const RETRACT_TRIP_STAGE: &str = RETRACT_TRIP_PROVENANCE.stage.label();

/// Resolution note of [`RetractTripCount`].
pub const RETRACT_TRIP_RESOLUTION: &str = RETRACT_TRIP_PROVENANCE.resolution_note;

/// The measurement contract of [`ToolpathStats::truncated_core_mm2`] — the
/// SINGLE source of truth the three prose constants below are derived from
/// (M1 slice 1; before it they were three independent strings that narration
/// and diagnostics concatenated, with nothing tying them to the code that
/// produced the number).
///
/// It is exactly the scallop ring cascade's residual, because that is where
/// the number comes from: [`crate::scallop::ScallopReport::PROVENANCE`].
pub const TRUNCATED_CORE_PROVENANCE: crate::measurement::MeasurementProvenance =
    crate::scallop::ScallopReport::PROVENANCE;

/// Measurement domain of [`ToolpathStats::truncated_core_mm2`].
///
/// It is a **projected** area — the shoelace area of the ring cascade's
/// residual polygons in XY — and therefore NOT comparable with 3D surface
/// area, dexel-top area or removed volume (non-negotiable rule 3).
pub const TRUNCATED_CORE_DOMAIN: &str = TRUNCATED_CORE_PROVENANCE.domain.label();

/// Pipeline stage [`ToolpathStats::truncated_core_mm2`] is measured at.
///
/// Generation, from the cascade's own geometry. A simulation cannot
/// reproduce it: material the toolpath never attempted to cut leaves no
/// trace in a cut record, which is exactly why the defect survived.
pub const TRUNCATED_CORE_STAGE: &str = TRUNCATED_CORE_PROVENANCE.stage.label();

/// Resolution of [`ToolpathStats::truncated_core_mm2`].
///
/// Not a grid measure: the residual is the ring polygons themselves, whose
/// vertices are decimated to `0.75 ×` the finish heightmap cell during the
/// cascade (`scallop.rs`). Exterior-shoelace: holes are not subtracted
/// (`MEASUREMENT_DOMAINS.md` X-5), so treat it as an upper bound. M4 §5b
/// closed X-5 with a hole-aware sibling rather than by changing this
/// figure — see [`UNTOUCHED_MATERIAL_PROVENANCE`].
pub const TRUNCATED_CORE_RESOLUTION: &str = TRUNCATED_CORE_PROVENANCE.resolution_note;

/// The measurement contract of [`ToolpathStats::untouched_material_mm2`]
/// (M4 §5b) — the SINGLE source [`UNTOUCHED_MATERIAL_DOMAIN`] and friends
/// derive from, exactly the pattern [`TRUNCATED_CORE_PROVENANCE`] set.
///
/// Same domain and stage as [`TRUNCATED_CORE_PROVENANCE`] — both are
/// exact shoelace areas over the same truncated-cascade polygons — but this
/// one is [`crate::scallop::ScallopReport::UNTOUCHED_PROVENANCE`], which
/// nets out holes where the other sums exteriors only.
pub const UNTOUCHED_MATERIAL_PROVENANCE: crate::measurement::MeasurementProvenance =
    crate::scallop::ScallopReport::UNTOUCHED_PROVENANCE;

/// Measurement domain of [`ToolpathStats::untouched_material_mm2`].
pub const UNTOUCHED_MATERIAL_DOMAIN: &str = UNTOUCHED_MATERIAL_PROVENANCE.domain.label();

/// Pipeline stage [`ToolpathStats::untouched_material_mm2`] is measured at.
pub const UNTOUCHED_MATERIAL_STAGE: &str = UNTOUCHED_MATERIAL_PROVENANCE.stage.label();

/// Resolution of [`ToolpathStats::untouched_material_mm2`]: the hole-aware
/// net area — see [`crate::scallop::ScallopReport::untouched_mm2`]'s doc.
pub const UNTOUCHED_MATERIAL_RESOLUTION: &str = UNTOUCHED_MATERIAL_PROVENANCE.resolution_note;

/// The measurement contract of
/// [`ToolpathStats::reached_uncut_estimate_mm2`] (M4 §5b) — the SINGLE
/// source [`REACHED_UNCUT_ESTIMATE_DOMAIN`] and friends derive from.
///
/// A DIFFERENT [`crate::measurement::MeasurementStage`] from
/// [`TRUNCATED_CORE_PROVENANCE`] / [`UNTOUCHED_MATERIAL_PROVENANCE`] —
/// [`crate::scallop::ScallopReport::STANDING_PROVENANCE`] — so
/// [`crate::measurement::MeasurementProvenance::comparable_to`] refuses to
/// treat this ESTIMATOR as interchangeable with either exact polygon area,
/// even though all three travel on the same `ToolpathStats`.
pub const REACHED_UNCUT_ESTIMATE_PROVENANCE: crate::measurement::MeasurementProvenance =
    crate::scallop::ScallopReport::STANDING_PROVENANCE;

/// Measurement domain of [`ToolpathStats::reached_uncut_estimate_mm2`].
pub const REACHED_UNCUT_ESTIMATE_DOMAIN: &str = REACHED_UNCUT_ESTIMATE_PROVENANCE.domain.label();

/// Pipeline stage [`ToolpathStats::reached_uncut_estimate_mm2`] is
/// measured at.
pub const REACHED_UNCUT_ESTIMATE_STAGE: &str = REACHED_UNCUT_ESTIMATE_PROVENANCE.stage.label();

/// Resolution of [`ToolpathStats::reached_uncut_estimate_mm2`]: an
/// estimator, not a polygon area — see
/// [`crate::scallop::ScallopReport::standing_mm2`]'s doc for the formula and
/// its stated limitations.
pub const REACHED_UNCUT_ESTIMATE_RESOLUTION: &str =
    REACHED_UNCUT_ESTIMATE_PROVENANCE.resolution_note;

/// Minimum clearance (mm) between `safe_z` and the top of the stock.
///
/// Rapids at `safe_z` must clear the uncut stock surface. The user-configured
/// `post.safe_z` is a legacy 2D-friendly default (often 10mm above work Z=0),
/// which sits *inside* the stock for any 3D job with stock extending above
/// that value. This clearance forces `safe_z` to at least `stock_top + 5mm`.
pub const SAFE_Z_CLEARANCE_MM: f64 = 5.0;

/// Apply the stock-clearance floor to a raw `post.safe_z` value.
///
/// Returns `max(raw_safe_z, stock_top + SAFE_Z_CLEARANCE_MM)`. Use this at the
/// single point where a [`HeightContext`] is built so that every operation and
/// downstream helper sees the same effective safe_z.
pub fn effective_safe_z(raw_safe_z: f64, stock_top: f64) -> f64 {
    raw_safe_z.max(stock_top + SAFE_Z_CLEARANCE_MM)
}

/// Named reference point for expressing heights relative to geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeightReference {
    StockTop,
    StockBottom,
    ModelTop,
    ModelBottom,
}

impl HeightReference {
    pub const ALL: &[HeightReference] = &[
        HeightReference::StockTop,
        HeightReference::StockBottom,
        HeightReference::ModelTop,
        HeightReference::ModelBottom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HeightReference::StockTop => "Stock Top",
            HeightReference::StockBottom => "Stock Bottom",
            HeightReference::ModelTop => "Model Top",
            HeightReference::ModelBottom => "Model Bottom",
        }
    }

    /// Resolve the reference Z value from context. Model refs fall back to stock.
    pub fn resolve_z(self, ctx: &HeightContext) -> f64 {
        match self {
            HeightReference::StockTop => ctx.stock_top_z,
            HeightReference::StockBottom => ctx.stock_bottom_z,
            HeightReference::ModelTop => ctx.model_top_z.unwrap_or(ctx.stock_top_z),
            HeightReference::ModelBottom => ctx.model_bottom_z.unwrap_or(ctx.stock_bottom_z),
        }
    }
}

/// An offset from a named reference point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReferenceOffset {
    pub reference: HeightReference,
    pub offset: f64,
}

/// Context needed to resolve heights -- stock/model geometry and post config.
#[derive(Debug, Clone, Copy)]
pub struct HeightContext {
    pub safe_z: f64,
    pub op_depth: f64,
    pub stock_top_z: f64,
    pub stock_bottom_z: f64,
    pub model_top_z: Option<f64>,
    pub model_bottom_z: Option<f64>,
}

impl HeightContext {
    /// Minimal context for tests / simple cases where only safe_z and op_depth matter.
    /// Stock spans 0 -> safe_z, no model.
    pub fn simple(safe_z: f64, op_depth: f64) -> Self {
        Self {
            safe_z,
            op_depth,
            stock_top_z: 0.0,
            stock_bottom_z: -op_depth,
            model_top_z: None,
            model_bottom_z: None,
        }
    }
}

/// Controls whether a height value is auto-computed, manually set, or relative to a reference.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum HeightMode {
    /// Auto-compute from stock/operation context.
    Auto,
    /// User-specified absolute Z value.
    Manual(f64),
    /// Offset from a named reference point (e.g. "5 mm from Stock Top").
    FromReference(ReferenceOffset),
}

impl HeightMode {
    /// Resolve to a concrete Z value given auto default and context.
    pub fn resolve_value(&self, auto_value: f64, ctx: &HeightContext) -> f64 {
        match self {
            HeightMode::Auto => auto_value,
            HeightMode::Manual(value) => *value,
            HeightMode::FromReference(r) => r.reference.resolve_z(ctx) + r.offset,
        }
    }

    pub fn is_auto(&self) -> bool {
        matches!(self, HeightMode::Auto)
    }
}

/// Five-level height system controlling vertical tool motion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeightsConfig {
    pub clearance_z: HeightMode,
    pub retract_z: HeightMode,
    pub feed_z: HeightMode,
    pub top_z: HeightMode,
    pub bottom_z: HeightMode,
}

impl Default for HeightsConfig {
    fn default() -> Self {
        Self {
            clearance_z: HeightMode::Auto,
            retract_z: HeightMode::Auto,
            feed_z: HeightMode::Auto,
            top_z: HeightMode::Auto,
            bottom_z: HeightMode::Auto,
        }
    }
}

impl HeightsConfig {
    /// Resolve all heights given stock/model/post context.
    ///
    /// F-028 (2026-05-25): `top_z` Auto now resolves to `ctx.stock_top_z`
    /// (the stock top in the operation's emission frame) instead of `0.0`.
    /// Pre-fix any project where stock top sat at world Z != 0 emitted 2.5D
    /// cuts at world Z=[-depth, 0] (i.e. below or above the actual stock),
    /// because face / pocket / profile / etc. fed `heights.top_z = 0` into
    /// their depth stepping. With the fix the depth stepping anchors to the
    /// actual stock top in the right frame (`world` for identity setups,
    /// `local` for non-identity setups — the caller passes the right
    /// `stock_top_z` per setup; see `session/compute.rs::compute` for the
    /// identity-vs-non-identity dispatch).
    ///
    /// `bottom_z` Auto remains `-op_depth.abs()` so depth-from-top semantics
    /// keep working for callers that haven't migrated; combining with the
    /// new `top_z` Auto yields `bottom_z = -op_depth` (unchanged when
    /// `stock_top_z == 0`, which is the common test/fixture case).
    pub fn resolve(&self, ctx: &HeightContext) -> ResolvedHeights {
        let retract = self.retract_z.resolve_value(ctx.safe_z, ctx);
        let top_z = self.top_z.resolve_value(ctx.stock_top_z, ctx);
        ResolvedHeights {
            clearance_z: self.clearance_z.resolve_value(retract + 10.0, ctx),
            retract_z: retract,
            feed_z: self.feed_z.resolve_value(retract - 2.0, ctx),
            top_z,
            bottom_z: self.bottom_z.resolve_value(top_z - ctx.op_depth.abs(), ctx),
            top_pinned: !self.top_z.is_auto(),
            bottom_pinned: !self.bottom_z.is_auto(),
        }
    }
}

/// Fully resolved (concrete) heights for a single operation.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedHeights {
    pub clearance_z: f64,
    pub retract_z: f64,
    pub feed_z: f64,
    pub top_z: f64,
    pub bottom_z: f64,
    /// True when `top_z` came from a user choice (Manual / FromReference)
    /// rather than Auto. Carried for ops that distinguish user intent from
    /// the Auto default; note adaptive3d deliberately ignores a pinned top
    /// (see `generate_adaptive3d` — roughing must start from the real
    /// material top or the simulator carries unplanned overhead).
    pub top_pinned: bool,
    /// True when `bottom_z` came from a user choice. adaptive3d honors
    /// this as a floor on its Z-level plan (audit 2026-06-12, finding 2).
    pub bottom_pinned: bool,
}

impl ResolvedHeights {
    /// The depth range: distance from top_z to bottom_z (positive value).
    pub fn depth(&self) -> f64 {
        (self.top_z - self.bottom_z).abs()
    }
}

/// Tool containment mode for machining boundary (re-export from core).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryContainment {
    #[default]
    Center,
    Inside,
    Outside,
}

/// How a machining boundary is derived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BoundarySource {
    /// Stock bounding rectangle (current default).
    #[default]
    Stock,
    /// 2D silhouette of the 3D model projected along the tool axis (Z-down).
    ModelSilhouette,
    /// Imported 2D geometry (DXF/SVG closed chains) — indices into the
    /// toolpath's polygon list.
    Geometry { polygon_indices: Vec<usize> },
    /// Selected STEP/CAD faces projected to XY.
    FaceSelection,
    /// Rest regions derived from another toolpath's rest-depth analysis
    /// (`AnnotatedToolpath::rest_regions`, populated by the pencil
    /// RestDepth detector). `source_toolpath_id` is the *stable id*
    /// (`ToolpathConfig.id`, not an index) of the toolpath whose cached
    /// generation result supplies the polygons — the regions are
    /// re-resolved from that result at generation time, never copied in
    /// here, so they always reflect the source's latest generation.
    DerivedRestRegions {
        source_toolpath_id: crate::ids::ToolpathId,
    },
}

impl BoundarySource {
    /// Sources with no extra configuration beyond picking them — safe for a
    /// simple combo box. `Geometry` needs a polygon-index picker,
    /// `FaceSelection` a face picker, and `DerivedRestRegions` a
    /// source-toolpath picker, so none of those three are listed here.
    pub const ALL_SIMPLE: &[BoundarySource] =
        &[BoundarySource::Stock, BoundarySource::ModelSilhouette];

    pub fn label(&self) -> &'static str {
        match self {
            BoundarySource::Stock => "Stock",
            BoundarySource::ModelSilhouette => "Model Silhouette",
            BoundarySource::Geometry { .. } => "Imported Geometry",
            BoundarySource::FaceSelection => "Face Selection",
            BoundarySource::DerivedRestRegions { .. } => "Rest Regions",
        }
    }
}

/// Full machining boundary configuration.
///
/// Can live on `StockConfig` (global default) or on individual `ToolpathEntry`
/// (per-toolpath override).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryConfig {
    pub enabled: bool,
    pub source: BoundarySource,
    pub containment: BoundaryContainment,
    /// Additional offset in mm (positive = expand, negative = shrink).
    /// Applied after source resolution, before tool-radius containment.
    pub offset: f64,
}

impl Default for BoundaryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            source: BoundarySource::Stock,
            containment: BoundaryContainment::Center,
            offset: 0.0,
        }
    }
}

/// Op-agnostic rest analysis: any toolpath can turn this on to run the
/// rest-depth detector (`rest_field::detect_rest_valleys`) against ITS OWN
/// tool as the fine cutter, using `reference_tool_id` (or the machined stock,
/// or a nominal ball — same resolution order pencil's rest-depth detector
/// already uses) as the reference. Before this config existed, only pencil's
/// `RestDepth` detector arm populated `rest_grid` / `rest_regions` on the
/// generated toolpath; this makes that analysis available to every family
/// (scallop, waterline, adaptive3d, ...) without generating a pencil
/// centerline toolpath at all — see `compute::execute`'s post-generation
/// rest-analysis pass for the generic wiring, and `pencil.rs::rest_depth_arm`
/// for the original detector this reuses.
///
/// Field defaults mirror `rest_field::RestFieldParams`'s own defaults
/// (`cell_mm` = 0.5, `min_valley_depth` = 0.05, `region_margin_mm` = 0.5) —
/// the two structs describe the same underlying algorithm from two call
/// sites (config vs. detector internals) and should stay numerically in sync.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestAnalysisConfig {
    pub enabled: bool,
    /// Real library tool whose geometry defines the rest reference. `None`
    /// falls back to the machined stock (when available) or a nominal ball,
    /// same resolution order as `PencilConfig::reference_tool_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_tool_id: Option<crate::compute::tool_config::ToolId>,
    /// XY grid cell size (mm) for the rest field. Smaller = finer regions.
    pub cell_mm: f64,
    /// Rest-depth threshold (mm): a cell counts as REST material once the
    /// reference floats more than this above the true surface.
    pub min_valley_depth: f64,
    /// Extra clearance (mm) added around detected rest regions beyond the
    /// generating toolpath's own tool radius, when dilating the mask into
    /// region polygons.
    pub region_margin_mm: f64,
    /// PR-7 (H2.5): offset stepover (mm) the ROUTING criterion assumes a
    /// downstream pencil fan would emit — `pencil ⟺ X_reach ≤ cap ×
    /// stepover` ([`crate::reach`]).
    ///
    /// `None` (the default, and what every project written before PR-7
    /// deserializes to) means **size it from the canonical reach policy**
    /// for THIS toolpath's own cutter, at [`Self::min_valley_depth`] — the
    /// same [`crate::reach::suggested_offset_stepover_mm`] call
    /// `UnifiedFinish`'s claims pipeline makes. Before PR-7 this pass took
    /// the detector's literal 0.5 mm default, which on a tapered tool
    /// describes no fan anything would emit.
    ///
    /// Set it explicitly only to model a SPECIFIC downstream fan — e.g. a
    /// pencil operation whose `offset_stepover` the operator has pinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_stepover_mm: Option<f64>,
    /// PR-7 (H2.5): offset passes per side that fan is permitted, the `cap`
    /// in the same criterion. `None` = the detector's own default (0 —
    /// centreline only, which the coverage cap FLOOR still widens to the
    /// band one pass actually works, see
    /// [`crate::reach::coverage_cap_passes`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_offset_passes: Option<usize>,
}

impl Default for RestAnalysisConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            reference_tool_id: None,
            cell_mm: 0.5,
            min_valley_depth: 0.05,
            region_margin_mm: 0.5,
            // Not a value: an instruction to ask the reach policy. See the
            // field docs.
            offset_stepover_mm: None,
            num_offset_passes: None,
        }
    }
}

/// Entry style for plunge replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DressupEntryStyle {
    None,
    Ramp,
    Helix,
}

impl DressupEntryStyle {
    /// Convert to core `EntryStyle` using parameters from the dressup config.
    /// Returns `None` for `DressupEntryStyle::None` (no entry transformation).
    pub fn to_core(self, cfg: &DressupConfig) -> Option<crate::dressup::EntryStyle> {
        use crate::dressup::EntryStyle;
        match self {
            Self::None => None,
            Self::Ramp => Some(EntryStyle::Ramp {
                max_angle_deg: cfg.ramp_angle,
            }),
            Self::Helix => Some(EntryStyle::Helix {
                radius: cfg.helix_radius,
                pitch: cfg.helix_pitch,
            }),
        }
    }
}

/// How the tool retracts between cutting passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetractStrategy {
    /// Always retract to retract_z (safest, default).
    Full,
    /// Retract just above the highest Z on nearby path + 2mm (faster).
    Minimum,
}

/// Default deviation budget (mm) for [`DressupConfig::segment_merge`].
fn default_segment_merge_tolerance() -> f64 {
    0.3
}

/// Configurable dressups applied after toolpath generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DressupConfig {
    pub entry_style: DressupEntryStyle,
    pub ramp_angle: f64,
    pub helix_radius: f64,
    pub helix_pitch: f64,
    pub dogbone: bool,
    pub dogbone_angle: f64,
    pub lead_in_out: bool,
    pub lead_radius: f64,
    /// F-040: Lead-in feed rate (mm/min). When `Some`, lead-in arc moves
    /// emitted by `apply_lead_in_out` use this rate (typically slower than
    /// cutting feed for a softer entry / cleaner dwell mark). When `None`,
    /// lead-in inherits the operation's primary `feed_rate` (pre-F-040
    /// behaviour). Default `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_in_feed_rate: Option<f64>,
    /// F-040: Lead-out feed rate (mm/min). Same fallback semantics —
    /// typically faster than cutting feed (chip-clear on exit). Default `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_out_feed_rate: Option<f64>,
    pub link_moves: bool,
    pub link_max_distance: f64,
    pub link_feed_rate: f64,
    pub arc_fitting: bool,
    pub arc_tolerance: f64,
    /// Phase 1 (accel-friendly toolpaths): merge dense runs of consecutive
    /// same-feed linear cut moves whose interior points lie within
    /// `segment_merge_tolerance` of the retained chord, so a low-acceleration
    /// controller can ramp to the commanded feed instead of stalling on
    /// sub-millimetre segments. Runs after arc-fitting. Default on for
    /// roughing; `#[serde(default)]` keeps older project files loadable.
    #[serde(default)]
    pub segment_merge: bool,
    /// Deviation budget (mm) for `segment_merge`. Sits between the generation
    /// tolerance and stock-to-leave (roughing leaves ≥0.5 mm). Default 0.3.
    #[serde(default = "default_segment_merge_tolerance")]
    pub segment_merge_tolerance: f64,
    pub feed_optimization: bool,
    pub feed_max_rate: f64,
    pub feed_ramp_rate: f64,
    pub optimize_rapid_order: bool,
    pub retract_strategy: RetractStrategy,
    /// When the air-cut filter may replace a run of in-air cutting with a
    /// retract bridge (`planning/unified_v3_design.md` §10).
    ///
    /// `Always` (default) is the historical behaviour: bridge every air
    /// run however short. That is a net loss whenever the retract round
    /// trip is longer than the air it skips, which on a rest-clearer is
    /// most of them — the unified rest-clearer's 1 634 emitted fragments
    /// became 15 373 after filtering, and the added bridges account for
    /// almost all of its 539 m of rapid travel.
    #[serde(default)]
    pub air_bridge_policy: crate::dressup::AirBridgePolicy,
}

impl Default for DressupConfig {
    fn default() -> Self {
        // Roadmap B.6 — three pure-flips: link_moves, feed_optimization,
        // optimize_rapid_order. All are wins for the typical case;
        // operations that can't tolerate them are stripped by
        // `normalize_for_op` (which `for_op` now also calls on construct).
        Self {
            entry_style: DressupEntryStyle::None,
            ramp_angle: 3.0,
            helix_radius: 2.0,
            helix_pitch: 1.0,
            dogbone: false,
            dogbone_angle: 90.0,
            lead_in_out: false,
            lead_radius: 2.0,
            lead_in_feed_rate: None,
            lead_out_feed_rate: None,
            link_moves: true,
            link_max_distance: 10.0,
            link_feed_rate: 500.0,
            arc_fitting: false,
            arc_tolerance: 0.05,
            segment_merge: false,
            segment_merge_tolerance: 0.3,
            feed_optimization: true,
            feed_max_rate: 3000.0,
            feed_ramp_rate: 200.0,
            optimize_rapid_order: true,
            retract_strategy: RetractStrategy::Full,
            air_bridge_policy: crate::dressup::AirBridgePolicy::default(),
        }
    }
}

impl DressupConfig {
    /// Smart defaults based on operation process role.
    pub fn for_role(role: super::catalog::UiProcessRole) -> Self {
        use super::catalog::UiProcessRole;
        let base = Self::default();
        match role {
            // Roadmap B.5 — Roughing operations get a Ramp entry by
            // default (was None, which gave every Pocket / Profile /
            // Adaptive / Face / Adaptive3d a vertical plunge). The
            // op-type override below promotes Adaptive/Adaptive3d to
            // Helix and strips back to None for Drill/Trace.
            UiProcessRole::Roughing => Self {
                entry_style: DressupEntryStyle::Ramp,
                arc_fitting: true,
                // Phase 1: roughing leaves ≥0.5 mm stock, so a 0.3 mm merge
                // deviation never touches the finish surface — pure win for
                // controller tracking. Finish/SemiFinish stay off (surface
                // fidelity).
                segment_merge: true,
                link_moves: true,
                optimize_rapid_order: true,
                ..base
            },
            UiProcessRole::SemiFinish => Self {
                entry_style: DressupEntryStyle::Ramp,
                arc_fitting: true,
                optimize_rapid_order: true,
                ..base
            },
            UiProcessRole::Finish => Self {
                entry_style: DressupEntryStyle::Ramp,
                lead_in_out: true,
                arc_fitting: true,
                optimize_rapid_order: true,
                ..base
            },
        }
    }

    /// Smart defaults based on the operation type. Uses `for_role` as a base
    /// and overrides per-op where the role-level defaults don't fit.
    pub fn for_op(op: super::catalog::OperationType) -> Self {
        use super::catalog::OperationType;
        let mut cfg = Self::for_role(op.spec().ui_process_role);
        // ProjectCurve traces 2D rings projected onto a surface. Ramp entries
        // would cut a straight diagonal across the surface next to each ring
        // start — and there can be hundreds of tiny rings. Use a direct plunge.
        if op == OperationType::ProjectCurve {
            cfg.entry_style = DressupEntryStyle::None;
            cfg.lead_in_out = false;
            // Link moves bridge separate path fragments at cutting depth.
            // For project_curve, fragments can be distant (different rings,
            // or gaps where the path leaves the mesh footprint), and bridging
            // them at depth carves phantom lines across the stock. Always
            // rapid-retract between fragments.
            cfg.link_moves = false;
        }
        // Apply the same per-op constraints fresh creates would otherwise
        // only see on project-file load. Without this, an MCP/GUI fresh
        // toolpath would carry e.g. `link_moves = true` (from the new
        // Default B.6) on a DropCutter where it's known to break
        // (Roadmap B.5/B.6).
        cfg.normalize_for_op(op);
        cfg
    }

    /// Normalize an existing dressup config for the given operation type,
    /// disabling any dressup that's geometrically incompatible with how the
    /// operation emits toolpaths. Intended as a one-shot migration on
    /// project load so the UI state and compute behaviour stay in lockstep.
    /// Returns `true` if anything changed, `false` if the config was already
    /// consistent.
    pub fn normalize_for_op(&mut self, op: super::catalog::OperationType) -> bool {
        use super::catalog::EntryStylePolicy;
        // Phase 1 (architectural refactor T5): the per-op decisions live
        // in the registry's `dressup_policy` field — ONE source shared
        // with the viz dressup panel. The op-specific rationale (phantom
        // diagonals on ProjectCurve/DropCutter, Roadmap B.5 entry
        // overrides, F-031 Adaptive3d planner↔simulator stamp parity)
        // is documented on the registry entries in compute/catalog.rs.
        let policy = op.registry_entry().dressup_policy;
        let mut changed = false;
        if policy.strip_all_reason.is_some() {
            if self.entry_style != DressupEntryStyle::None {
                self.entry_style = DressupEntryStyle::None;
                changed = true;
            }
            if self.lead_in_out {
                self.lead_in_out = false;
                changed = true;
            }
            if self.link_moves {
                self.link_moves = false;
                changed = true;
            }
        }
        match policy.entry {
            EntryStylePolicy::ForceNone => {
                if self.entry_style != DressupEntryStyle::None {
                    self.entry_style = DressupEntryStyle::None;
                    changed = true;
                }
            }
            EntryStylePolicy::PreferHelix => {
                if self.entry_style == DressupEntryStyle::Ramp {
                    self.entry_style = DressupEntryStyle::Helix;
                    changed = true;
                }
            }
            EntryStylePolicy::AnyEntry => {}
        }
        changed
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    fn test_ctx() -> HeightContext {
        HeightContext {
            safe_z: 10.0,
            op_depth: 5.0,
            stock_top_z: 25.0,
            stock_bottom_z: 0.0,
            model_top_z: Some(20.0),
            model_bottom_z: Some(2.0),
        }
    }

    #[test]
    fn auto_resolve_top_z_follows_ctx_stock_top_z() {
        // F-028: Auto top_z now anchors to ctx.stock_top_z (was hardcoded 0).
        let cfg = HeightsConfig::default();
        let ctx = test_ctx();
        let h = cfg.resolve(&ctx);
        // retract = safe_z = 10
        assert!((h.retract_z - 10.0).abs() < 1e-9);
        // clearance = retract + 10 = 20
        assert!((h.clearance_z - 20.0).abs() < 1e-9);
        // feed = retract - 2 = 8
        assert!((h.feed_z - 8.0).abs() < 1e-9);
        // F-028: top_z Auto = ctx.stock_top_z = 25 (was 0 pre-fix)
        assert!((h.top_z - 25.0).abs() < 1e-9);
        // F-028: bottom_z Auto = top_z - op_depth = 25 - 5 = 20 (was -5 pre-fix)
        assert!((h.bottom_z - 20.0).abs() < 1e-9);
    }

    #[test]
    fn auto_resolve_top_z_zero_when_ctx_stock_top_z_zero() {
        // Backwards-compat for callers that build `HeightContext::simple` /
        // pre-F-028 fixtures with `stock_top_z = 0.0`: Auto top_z still
        // resolves to 0 in that frame, and bottom_z auto = -op_depth.
        let cfg = HeightsConfig::default();
        let ctx = HeightContext::simple(10.0, 5.0);
        let h = cfg.resolve(&ctx);
        assert!((h.top_z - 0.0).abs() < 1e-9);
        assert!((h.bottom_z - (-5.0)).abs() < 1e-9);
    }

    #[test]
    fn manual_passthrough() {
        let cfg = HeightsConfig {
            clearance_z: HeightMode::Manual(50.0),
            retract_z: HeightMode::Manual(30.0),
            feed_z: HeightMode::Manual(28.0),
            top_z: HeightMode::Manual(1.0),
            bottom_z: HeightMode::Manual(-10.0),
        };
        let h = cfg.resolve(&test_ctx());
        assert!((h.clearance_z - 50.0).abs() < 1e-9);
        assert!((h.retract_z - 30.0).abs() < 1e-9);
        assert!((h.feed_z - 28.0).abs() < 1e-9);
        assert!((h.top_z - 1.0).abs() < 1e-9);
        assert!((h.bottom_z - (-10.0)).abs() < 1e-9);
    }

    #[test]
    fn from_reference_stock_top() {
        let cfg = HeightsConfig {
            top_z: HeightMode::FromReference(ReferenceOffset {
                reference: HeightReference::StockTop,
                offset: -2.0,
            }),
            ..HeightsConfig::default()
        };
        let h = cfg.resolve(&test_ctx());
        // stock_top = 25, offset = -2 => 23
        assert!((h.top_z - 23.0).abs() < 1e-9);
    }

    #[test]
    fn from_reference_stock_bottom() {
        let cfg = HeightsConfig {
            bottom_z: HeightMode::FromReference(ReferenceOffset {
                reference: HeightReference::StockBottom,
                offset: 1.0,
            }),
            ..HeightsConfig::default()
        };
        let h = cfg.resolve(&test_ctx());
        // stock_bottom = 0, offset = 1 => 1
        assert!((h.bottom_z - 1.0).abs() < 1e-9);
    }

    #[test]
    fn from_reference_model_top() {
        let cfg = HeightsConfig {
            top_z: HeightMode::FromReference(ReferenceOffset {
                reference: HeightReference::ModelTop,
                offset: -3.0,
            }),
            ..HeightsConfig::default()
        };
        let h = cfg.resolve(&test_ctx());
        // model_top = 20, offset = -3 => 17
        assert!((h.top_z - 17.0).abs() < 1e-9);
    }

    #[test]
    fn model_ref_fallback_to_stock() {
        let ctx = HeightContext {
            model_top_z: None,
            model_bottom_z: None,
            ..test_ctx()
        };
        let cfg = HeightsConfig {
            top_z: HeightMode::FromReference(ReferenceOffset {
                reference: HeightReference::ModelTop,
                offset: 0.0,
            }),
            bottom_z: HeightMode::FromReference(ReferenceOffset {
                reference: HeightReference::ModelBottom,
                offset: 0.0,
            }),
            ..HeightsConfig::default()
        };
        let h = cfg.resolve(&ctx);
        // falls back to stock_top=25 and stock_bottom=0
        assert!((h.top_z - 25.0).abs() < 1e-9);
        assert!((h.bottom_z - 0.0).abs() < 1e-9);
    }

    #[test]
    fn serde_roundtrip_all_variants() {
        let auto = HeightMode::Auto;
        let manual = HeightMode::Manual(5.0);
        let from_ref = HeightMode::FromReference(ReferenceOffset {
            reference: HeightReference::StockTop,
            offset: -2.5,
        });

        for mode in [auto, manual, from_ref] {
            let json = serde_json::to_string(&mode).unwrap();
            let restored: HeightMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, restored);
        }
    }

    #[test]
    fn backward_compat_old_json() {
        // Old files produce these JSON forms
        let auto: HeightMode = serde_json::from_str(r#"{"mode":"auto"}"#).unwrap();
        assert!(auto.is_auto());

        let manual: HeightMode = serde_json::from_str(r#"{"mode":"manual","value":5.0}"#).unwrap();
        assert_eq!(manual, HeightMode::Manual(5.0));
    }

    #[test]
    fn height_reference_all_exhaustive() {
        assert_eq!(
            HeightReference::ALL.len(),
            4,
            "HeightReference::ALL out of sync with enum"
        );
        use std::collections::HashSet;
        let refs: HashSet<_> = HeightReference::ALL.iter().collect();
        assert_eq!(
            refs.len(),
            HeightReference::ALL.len(),
            "HeightReference::ALL has duplicates"
        );
    }

    /// Phase 1 T5: the per-op dressup policy table, pinned. The registry
    /// field forces every NEW op to decide its policy at compile time;
    /// this pin makes changing an EXISTING op's policy loud. Mirrors the
    /// pre-registry predicates exactly (no behavior change).
    #[test]
    fn dressup_policy_table_is_pinned() {
        use super::super::catalog::{EntryStylePolicy, OperationType};
        for &op in OperationType::ALL {
            let policy = op.registry_entry().dressup_policy;
            match op {
                // UnifiedFinish joined the strip-all set 2026-07-09 (P2.f
                // Task 2): role-default Ramp entries carved diagonal
                // trenches across the wanaka terrain on a fresh MCP-added
                // op — same failure mode as DropCutter's documented one.
                OperationType::ProjectCurve
                | OperationType::DropCutter
                | OperationType::UnifiedFinish => {
                    assert!(policy.strip_all_reason.is_some(), "{op:?}: strip-all");
                    assert_eq!(policy.entry, EntryStylePolicy::AnyEntry);
                }
                OperationType::Drill | OperationType::Trace | OperationType::Adaptive3d => {
                    assert!(policy.strip_all_reason.is_none(), "{op:?}");
                    assert_eq!(policy.entry, EntryStylePolicy::ForceNone, "{op:?}");
                }
                OperationType::Adaptive => {
                    assert!(policy.strip_all_reason.is_none());
                    assert_eq!(policy.entry, EntryStylePolicy::PreferHelix);
                }
                _ => {
                    // Pre-registry these fell through the predicate
                    // matches untouched.
                    assert!(
                        policy.strip_all_reason.is_none(),
                        "{op:?}: expected ANY_DRESSUP"
                    );
                    assert_eq!(policy.entry, EntryStylePolicy::AnyEntry, "{op:?}");
                }
            }
        }
    }

    /// Phase 1 T5: normalize_for_op applies the registry policy with the
    /// pre-registry semantics — strip-all clears entry+lead+link;
    /// force-no-entry clears entry ONLY; prefer-helix upgrades Ramp and
    /// leaves Helix/None alone; unrestricted ops pass through untouched.
    #[test]
    fn normalize_for_op_applies_registry_policy() {
        use super::super::catalog::OperationType;

        let dirty = || DressupConfig {
            entry_style: DressupEntryStyle::Ramp,
            lead_in_out: true,
            link_moves: true,
            ..DressupConfig::default()
        };

        // Strip-all: ProjectCurve/DropCutter/UnifiedFinish clear all three.
        for op in [
            OperationType::ProjectCurve,
            OperationType::DropCutter,
            OperationType::UnifiedFinish,
        ] {
            let mut cfg = dirty();
            assert!(cfg.normalize_for_op(op));
            assert_eq!(cfg.entry_style, DressupEntryStyle::None, "{op:?}");
            assert!(!cfg.lead_in_out, "{op:?}");
            assert!(!cfg.link_moves, "{op:?}");
            // Idempotent.
            assert!(!cfg.normalize_for_op(op));
        }

        // Force-no-entry: entry cleared, lead/link untouched.
        for op in [
            OperationType::Drill,
            OperationType::Trace,
            OperationType::Adaptive3d,
        ] {
            let mut cfg = dirty();
            assert!(cfg.normalize_for_op(op));
            assert_eq!(cfg.entry_style, DressupEntryStyle::None, "{op:?}");
            assert!(cfg.lead_in_out, "{op:?}: lead-in/out must survive");
            assert!(cfg.link_moves, "{op:?}: link moves must survive");
        }

        // Prefer-helix: Ramp upgrades, Helix and None pass through.
        let mut cfg = dirty();
        assert!(cfg.normalize_for_op(OperationType::Adaptive));
        assert_eq!(cfg.entry_style, DressupEntryStyle::Helix);
        assert!(!cfg.normalize_for_op(OperationType::Adaptive));
        cfg.entry_style = DressupEntryStyle::None;
        assert!(!cfg.normalize_for_op(OperationType::Adaptive));

        // Unrestricted op: nothing changes.
        let mut cfg = dirty();
        assert!(!cfg.normalize_for_op(OperationType::Pocket));
        assert_eq!(cfg.entry_style, DressupEntryStyle::Ramp);
        assert!(cfg.lead_in_out && cfg.link_moves);
    }
}
