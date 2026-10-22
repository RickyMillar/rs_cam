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

impl StockSource {
    /// Does an operation with this source read the simulated machined stock?
    ///
    /// B19 (operator-level ruling 2026-10-15): a `Fresh` operation reads
    /// only the board, so its generation is a pure function of the project
    /// file. The generate step captures the `prior_stocks` snapshot only
    /// when this is `true`, and every stage that reads the machined stock
    /// (the generator seed, the air-cut filter, the entry replay, the
    /// entry-descent split, the result stamp) reads that one capture.
    #[must_use]
    pub const fn reads_machined_stock(self) -> bool {
        match self {
            Self::Fresh => false,
            Self::FromRemainingStock => true,
        }
    }
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
/// W5 item (a): the struct SERIALISES, so every surface that reports a block
/// renders one shape. Six sites used to hand-build the object from a
/// `serde_json::json!` literal, and one of them dropped `message`. Nothing
/// reads the shape back, so `Deserialize` is not derived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    pub const ALL: &[HeightReference] = &crate::all_variants!(HeightReference:
        StockTop, StockBottom, ModelTop, ModelBottom,
    );

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
    /// The resolved floor of a Pocket, Profile, Zigzag or Adaptive 2D
    /// (geometry selection P3.2). `None` for every other op. The session
    /// fills it (`ProjectSession::toolpath_heights`); the Auto bottom reads
    /// it first ([`HeightsConfig::auto_bottom_z`]).
    pub floor: Option<crate::compute::floor::FloorRef>,
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
            floor: None,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    ///
    /// R3 (Corne case, 2026-09-18): on a context with NO depth
    /// (`op_depth == 0`, the `DepthSemantics::None` family) the Auto bottom
    /// is the MODEL bottom, floored at the stock bottom. Before this the
    /// zero-depth arithmetic gave `bottom_z = top_z - 0 = stock top`, so an
    /// Auto waterline laddered ONE level at the stock top, above the mesh,
    /// and an Auto `UnifiedFinish` pinned its very-steep band to the rim
    /// (`planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §4.1).
    /// [`Self::auto_bottom_z`] is the rule; the Heights tab displays the
    /// same resolver, so display and generation agree.
    ///
    /// The Auto TOP stays at the stock top for every depth. The session's
    /// entry-descent pass reads the emission-frame STOCK top as its
    /// fresh-stock ceiling, raised to `heights.top_z` only when a pinned top
    /// sits above it (`crate::dressup::fresh_stock_top_z`, G-PECKSPLIT). Every
    /// link ceiling reads the same rule (ruling R1). It used to read `heights.top_z` alone, and a top pinned
    /// at the model top rapided a fresh-stock rough into the overhead above
    /// the model (Wanaka Back Rough, 1.47 mm).
    ///
    /// K3: the resolver does not read the obstacles of the setup. The
    /// retract and the clearance stay at the op's own values; the per-op
    /// obstacle step lifts only the moves that cross an obstacle to the
    /// obstacle plane (`compute::obstacle::clip_annotated_to_obstacles`).
    ///
    /// P3.1: `pub(crate)`. Every surface outside the crate reads
    /// `ProjectSession::toolpath_heights`, which applies the face top.
    pub(crate) fn resolve(&self, ctx: &HeightContext) -> ResolvedHeights {
        let retract = self.retract_z.resolve_value(ctx.safe_z, ctx);
        let top_z = self.top_z.resolve_value(ctx.stock_top_z, ctx);
        ResolvedHeights {
            clearance_z: self.clearance_z.resolve_value(retract + 10.0, ctx),
            retract_z: retract,
            feed_z: self.feed_z.resolve_value(retract - 2.0, ctx),
            top_z,
            bottom_z: self
                .bottom_z
                .resolve_value(Self::auto_bottom_z(top_z, ctx), ctx),
            top_pinned: !self.top_z.is_auto(),
            bottom_pinned: !self.bottom_z.is_auto(),
        }
    }

    /// The Auto bottom for a resolved `top_z` (R3).
    ///
    /// * A context with a floor (P3.2: Pocket, Profile, Zigzag, Adaptive 2D)
    ///   gives the floor: `FloorRef::z(top_z)`. The four ops refuse a pinned
    ///   bottom (`ToolpathConfig::floor_conflict`), so their
    ///   `ResolvedHeights::bottom_z` IS the floor
    ///   (`the_floor_reaches_the_motion_g_bottompin`). This arm comes first,
    ///   so a floor never falls into the zero-depth arm below.
    /// * A context WITH a depth keeps the F-028 arithmetic exactly:
    ///   `top_z - op_depth`.
    /// * A ZERO-depth context has no dial to subtract, so the floor is the
    ///   model bottom when the context carries a model, else the stock
    ///   bottom; either way never below the stock bottom. `top_z` is not
    ///   read on this arm: a pinned top below the model bottom yields an
    ///   inverted range, which the waterline ladder reports as empty.
    pub fn auto_bottom_z(top_z: f64, ctx: &HeightContext) -> f64 {
        if let Some(floor) = ctx.floor {
            floor.z(top_z)
        } else if ctx.op_depth.abs() < ZERO_DEPTH_EPSILON {
            ctx.model_bottom_z
                .map_or(ctx.stock_bottom_z, |model_bottom| {
                    model_bottom.max(ctx.stock_bottom_z)
                })
        } else {
            top_z - ctx.op_depth.abs()
        }
    }
}

/// Below this an `op_depth` counts as "no depth dial" for
/// [`HeightsConfig::auto_bottom_z`].
pub const ZERO_DEPTH_EPSILON: f64 = 1e-9;

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

// F033: `ResolvedHeights::depth()` returned `(top - bottom).abs()`, so an
// inverted stack read as a positive depth. It had no caller and is deleted.
// A depth is `top_z - bottom_z`; [`height_order`] keeps it positive, and a
// reader that needs the span computes it.

/// One break of the height-plane order (F033, operator ruling 2).
///
/// The order is clearance >= retract >= feed >= top > bottom, with the
/// anchors of [`height_order`]. An op whose ladder includes its top
/// (F033b) takes top >= bottom. Each kind has ONE severity
/// ([`Self::refuses`]). The GUI badge, the diagnostics, the edit doors,
/// generation and the export door read this one value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeightOrderViolation {
    /// Clearance Z is below Retract Z.
    ClearanceBelowRetract { clearance_z: f64, retract_z: f64 },
    /// Retract Z is below the anchored Top Z: the rapids within the
    /// operation go through the material.
    RetractBelowTop { retract_z: f64, top_z: f64 },
    /// Retract Z is below Feed Z.
    RetractBelowFeed { retract_z: f64, feed_z: f64 },
    /// Feed Z is below the anchored Top Z.
    FeedBelowTop { feed_z: f64, top_z: f64 },
    /// The bottom is at or above the anchored Top Z: the ladder is empty
    /// or inverted. On an op whose ladder includes its top (F033b), only a
    /// bottom above the top.
    BottomNotBelowTop { bottom_z: f64, top_z: f64 },
}

impl HeightOrderViolation {
    /// Every kind, for a test that walks them. `all_variants!` cannot
    /// build a variant with fields, so the list holds one example each.
    pub const EXAMPLES: [Self; 5] = [
        Self::ClearanceBelowRetract {
            clearance_z: 0.0,
            retract_z: 1.0,
        },
        Self::RetractBelowTop {
            retract_z: 0.0,
            top_z: 1.0,
        },
        Self::RetractBelowFeed {
            retract_z: 0.0,
            feed_z: 1.0,
        },
        Self::FeedBelowTop {
            feed_z: 0.0,
            top_z: 1.0,
        },
        Self::BottomNotBelowTop {
            bottom_z: 1.0,
            top_z: 0.0,
        },
    ];

    /// True when this violation refuses the edit, generation and export.
    /// THE severity split: change it here and every surface follows.
    ///
    /// Operator ruling 2 refuses an inverted stack. The retract, the top and
    /// the bottom reach the emitted motion, so a break between them refuses.
    /// A clearance below the retract refuses too: the ruling names the
    /// whole stack, and the GUI badge rated it DANGER before F033. The
    /// generators do not read `feed_z` (nor `clearance_z`; both reach only
    /// the debug trace), and the auto Feed Z is `retract - 2`, so a Retract
    /// pinned near the top puts the auto feed under the top with no error on
    /// the feed row. A feed-plane break is therefore a caution only.
    #[must_use]
    pub const fn refuses(self) -> bool {
        match self {
            Self::ClearanceBelowRetract { .. }
            | Self::RetractBelowTop { .. }
            | Self::BottomNotBelowTop { .. } => true,
            Self::RetractBelowFeed { .. } | Self::FeedBelowTop { .. } => false,
        }
    }

    /// True when `self` and `other` are the same kind (the Z values may
    /// differ). An edit door compares kinds.
    #[must_use]
    pub fn same_kind(self, other: Self) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }
}

impl std::fmt::Display for HeightOrderViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::ClearanceBelowRetract {
                clearance_z,
                retract_z,
            } => write!(
                f,
                "Clearance Z ({clearance_z:.2}) is below Retract Z ({retract_z:.2}). \
                 Raise the Clearance Z or lower the Retract Z"
            ),
            Self::RetractBelowTop { retract_z, top_z } => write!(
                f,
                "Retract Z ({retract_z:.2}) is below Top Z ({top_z:.2}). Rapid moves would \
                 go through the material. Raise the Retract Z or lower the Top Z"
            ),
            Self::RetractBelowFeed { retract_z, feed_z } => write!(
                f,
                "Retract Z ({retract_z:.2}) is below Feed Z ({feed_z:.2}). Retract moves \
                 do not clear the approach height"
            ),
            Self::FeedBelowTop { feed_z, top_z } => write!(
                f,
                "Feed Z ({feed_z:.2}) is below Top Z ({top_z:.2}). The approach height is \
                 inside the material"
            ),
            Self::BottomNotBelowTop { bottom_z, top_z } => write!(
                f,
                "Bottom Z ({bottom_z:.2}) is not below Top Z ({top_z:.2}). No material is \
                 cut. Lower the Bottom Z or raise the Top Z"
            ),
        }
    }
}

/// THE height-plane order rule (F033, operator ruling 2): the violations of
/// `h` for an operation of type `op`.
///
/// Anchors:
/// - The Top Z is the top the generator starts from. An op that starts on
///   the stock box (`Capabilities::top_on_stock_box`, Adaptive 3D) ignores
///   a pinned top, so its top is `stock_top_z` (R7, Corne case
///   2026-09-18). With no `stock_top_z` the anchor is `h.top_z`.
/// - The bottom counts only where it reaches the motion
///   (`FloorOwner::HeightsBottom` and `FloorOwner::OpFloor`). On an
///   `OpDial` op a pinned bottom reaches no emitted move (F1.19), so it is
///   not checked.
/// - A bottom at the top refuses, because the first level of a 2.5D ladder
///   is one pass below the top. An op whose ladder includes its top
///   (`Capabilities::ladder_includes_top`, F033b: Waterline, Unified Finish)
///   cuts one level there, so only a bottom above the top refuses.
///
/// The GUI badge, the diagnostics adapter, the edit doors, generation and
/// the export door call this function or [`crate::session::ToolpathHeights::height_order`],
/// which calls it. No surface keeps a second comparison.
#[must_use]
pub fn height_order(
    op: crate::compute::catalog::OperationType,
    h: &ResolvedHeights,
    stock_top_z: Option<f64>,
) -> Vec<HeightOrderViolation> {
    use crate::compute::catalog::FloorOwner;
    let caps = op.capabilities();
    let top_z = match stock_top_z {
        Some(stock_top) if caps.top_on_stock_box => stock_top,
        Some(_) | None => h.top_z,
    };
    let mut out = Vec::new();
    if h.clearance_z < h.retract_z {
        out.push(HeightOrderViolation::ClearanceBelowRetract {
            clearance_z: h.clearance_z,
            retract_z: h.retract_z,
        });
    }
    if h.retract_z < top_z {
        out.push(HeightOrderViolation::RetractBelowTop {
            retract_z: h.retract_z,
            top_z,
        });
    }
    if h.retract_z < h.feed_z {
        out.push(HeightOrderViolation::RetractBelowFeed {
            retract_z: h.retract_z,
            feed_z: h.feed_z,
        });
    }
    if h.feed_z < top_z {
        out.push(HeightOrderViolation::FeedBelowTop {
            feed_z: h.feed_z,
            top_z,
        });
    }
    let bottom_reaches_motion = match caps.floor_owner {
        FloorOwner::HeightsBottom | FloorOwner::OpFloor => true,
        FloorOwner::OpDial => false,
    };
    // F033b: a ladder that includes its top cuts one level at top = bottom.
    let inverted = if caps.ladder_includes_top {
        h.bottom_z > top_z
    } else {
        h.bottom_z >= top_z
    };
    if bottom_reaches_motion && inverted {
        out.push(HeightOrderViolation::BottomNotBelowTop {
            bottom_z: h.bottom_z,
            top_z,
        });
    }
    out
}

/// The refusing violations of `after` whose kind `before` does not hold.
///
/// An edit door refuses an edit that ADDS a refusing break. A stack that is
/// already broken (a stock change moved the top under a pinned retract)
/// still takes an edit that does not add a break, so the operator can fix
/// it one plane at a time. Generation and export refuse every refusing
/// break.
#[must_use]
pub fn added_refusals(
    before: &[HeightOrderViolation],
    after: &[HeightOrderViolation],
) -> Vec<HeightOrderViolation> {
    after
        .iter()
        .copied()
        .filter(|v| v.refuses() && !before.iter().any(|b| b.same_kind(*v)))
        .collect()
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
    /// The closed regions of a selection (geometry selection P2.5): the
    /// loops, circles and picked faces that the selection names, nested by
    /// containment, then joined into one union, holes kept. It replaces
    /// `ModelOutline` (`Selection::closed_shapes_of(model)`) and
    /// `FaceSelection` (a selection of face picks).
    ///
    /// A rule or a pick on `SourceRef::ToolpathModel` reads the model of
    /// the toolpath that owns this boundary. `SourceRef::Model(id)` reads
    /// any project model: a 3D rough on a terrain STL can take its
    /// boundary from a DXF.
    ///
    /// The selection resolves through the one door of an op selection
    /// (`resolve`, the setup frame, `view` with `ItemUse::Regions`).
    /// Generation refuses when the selection resolves to no closed region,
    /// names a model that does not exist, or holds a lost pick. It never
    /// falls back to the stock.
    ///
    /// Like [`Self::DerivedRestRegions`] this resolves to a SET of
    /// polygons, one per disjoint region, through the same `RegionSet`
    /// path.
    ///
    /// `holes` selects the opposite regions. When it is true, the regions
    /// are the HOLES of the union: one region per hole ring. A shape inside
    /// a hole (an island) is cut out of that region. An edge-band model
    /// then gives the area inside the band, and the toolpath stays off the
    /// band. Generation refuses when the union has no hole.
    Selection {
        selection: crate::selection::Selection,
        #[serde(default)]
        holes: bool,
    },
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
    /// One tier's islands from a multi-tool **tier map**
    /// (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` Phase O item 2).
    ///
    /// This variant carries the **recipe**, never the geometry — the same
    /// call [`Self::DerivedRestRegions`] makes, and for the same two reasons:
    /// a project file must store what to compute rather than a snapshot of a
    /// computation, and the regions must reflect the mesh and the ladder as
    /// they are at generation time. The islands are re-derived through
    /// [`crate::maps::tier_map_cache::cached_tier_map`], so the `k` sibling ops one
    /// plan emits share a single grid walk rather than paying `k` of them.
    ///
    /// Unlike `DerivedRestRegions` this has **no source-toolpath
    /// dependency**: it derives from the mesh and the ladder alone, so an op
    /// carrying it never waits on a prior generation and never enters the
    /// `AwaitingPriorStock` arms. (Such an op is normally *also*
    /// `StockSource::FromRemainingStock`, which is a separate dependency on
    /// simulated stock and is enforced separately.)
    ///
    /// `tier` is always ≥ 1. Tier 0 is the complement — the coarse tool
    /// sweeps its territory as one pass and needs no boundary — so
    /// [`crate::maps::tier_islands::TierIslands`] publishes no set for it and this
    /// variant is never emitted with `tier: 0`.
    PlannedTierRegions {
        /// The FULL ladder, coarse → fine, as session tool ids. The whole
        /// ladder is needed even to resolve one tier: a tier label is "the
        /// coarsest tool on THIS ladder that holds this cell", which is not
        /// a statement any subset can reproduce.
        tool_ids: Vec<crate::ids::ToolId>,
        /// Which tier's islands bound this op (≥ 1).
        tier: u8,
        /// Tier-map planning resolution (mm). Plan at 0.3–0.6; a full-grid
        /// drop-cutter map at 0.15 costs ~125 s per tool on a 200 mm board
        /// (`crate::maps::tier_map`'s module doc).
        cell_mm: f64,
        /// Residual tolerance (mm) — see [`crate::maps::tier_map::TierMapParams`].
        tolerance_mm: f64,
        /// Grid padding (mm) beyond the finest tool's envelope.
        margin_mm: f64,
        /// How the raw residual is treated before the tolerance comparison.
        treatment: crate::maps::tier_map::ResidualTreatment,
        /// Island close / min-area / overlap / cap dials.
        islands: crate::maps::tier_islands::TierIslandParams,
    },
}

impl BoundarySource {
    pub fn label(&self) -> &'static str {
        match self {
            BoundarySource::Stock => "Stock",
            BoundarySource::ModelSilhouette => "Model Silhouette",
            BoundarySource::Selection { .. } => "Selection",
            BoundarySource::DerivedRestRegions { .. } => "Rest Regions",
            BoundarySource::PlannedTierRegions { .. } => "Planned Tier Regions",
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

impl BoundaryConfig {
    /// The creation-time boundary of a 3D operation on a mesh model: the
    /// model silhouette, `Center` containment, expanded by one tool
    /// diameter (R2, `planning/corne_case_analysis_2026-09-18/ANALYSIS.md`
    /// §5). Roadmap B.7 enabled the silhouette so the cutter does not sweep
    /// the whole stock on a small part in oversized stock. Under `Center`
    /// containment the cutter centre stays inside the polygon. A silhouette
    /// with no offset therefore keeps the cutter one radius short of every
    /// outer face. The diameter offset lets the cutter pass the outer face
    /// with one radius of clearance.
    ///
    /// `offset` stores a NUMBER, a copy of `tool_diameter_mm` at creation.
    /// A later tool change does not update it. Both creation doors (the
    /// GUI controller and the MCP `add_toolpath` door) call this with the
    /// diameter of the tool they bind.
    #[must_use]
    pub fn for_3d_op(tool_diameter_mm: f64) -> Self {
        Self {
            enabled: true,
            source: BoundarySource::ModelSilhouette,
            containment: BoundaryContainment::Center,
            offset: tool_diameter_mm,
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
/// Field defaults are READ FROM `rest_field::RestFieldParams::default()`,
/// not restated here — the two structs describe the same underlying
/// algorithm from two call sites (config vs. detector internals), so the
/// detector owns the numbers and this config copies them.
///
/// CMP-16: both files used to write `cell_mm` = 0.5,
/// `min_valley_depth` = 0.05 and `region_margin_mm` = 0.5 as separate
/// literals, under a doc line that said they "should stay numerically in
/// sync", and no test compared them. The consumer proves the coupling is
/// real: `execute/dressup_apply.rs` builds one `RestFieldParams` from
/// this config's three fields and takes `num_offset_passes_cap` and
/// `min_cut_length` from `RestFieldParams::default()` in the same
/// literal.
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
    /// generating toolpath's own **CUSP (tip-sphere) radius**, when dilating
    /// the mask into region polygons.
    ///
    /// F1 (2026-08-23): the base was the ENVELOPE radius until then, which
    /// on a tapered ball is the shank — 3.5 mm of dilation on the shipped
    /// Ø1-tip / Ø6-shank taper, enough to weld dendritic islands into one
    /// region. See [`crate::surface::rest_field::RestFieldParams::region_margin_mm`].
    pub region_margin_mm: f64,
    /// PR-7 (H2.5): offset stepover (mm) the ROUTING criterion assumes a
    /// downstream pencil fan would emit — `pencil ⟺ X_reach ≤ cap ×
    /// stepover` ([`crate::surface::reach`]).
    ///
    /// `None` (the default, and what every project written before PR-7
    /// deserializes to) means **size it from the canonical reach policy**
    /// for THIS toolpath's own cutter, at [`Self::min_valley_depth`] — the
    /// same [`crate::surface::reach::suggested_offset_stepover_mm`] call
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
    /// [`crate::surface::reach::coverage_cap_passes`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_offset_passes: Option<usize>,
}

impl Default for RestAnalysisConfig {
    fn default() -> Self {
        // CMP-16: the detector's own defaults, read rather than restated.
        let detector = crate::surface::rest_field::RestFieldParams::default();
        Self {
            enabled: false,
            reference_tool_id: None,
            cell_mm: detector.cell_mm,
            min_valley_depth: detector.min_valley_depth,
            region_margin_mm: detector.region_margin_mm,
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
    ///
    /// The helix radius is the one [`DressupConfig::helix_radius_for`]
    /// resolves for `cutter`: the rule or the operator value, capped at the
    /// flat bottom (G10 Q6).
    pub fn to_core(
        self,
        cfg: &DressupConfig,
        cutter: &dyn crate::tool::MillingCutter,
    ) -> Option<crate::dressup::EntryStyle> {
        use crate::dressup::EntryStyle;
        match self {
            Self::None => None,
            Self::Ramp => Some(EntryStyle::Ramp {
                max_angle_deg: cfg.ramp_angle,
            }),
            Self::Helix => Some(EntryStyle::Helix {
                radius: cfg.helix_radius_for(cutter).emitted_mm,
                pitch: cfg.helix_pitch,
            }),
        }
    }
}

/// The dressup ramp entry angle (degrees) a new toolpath starts with.
///
/// Repo rule, no source (G10). No wood or router source prints a ramp
/// angle (ruling Q9); the card says so.
pub const DRESSUP_RAMP_ANGLE_DEG: f64 = 3.0;

/// The dressup helix entry pitch (mm per turn) a new toolpath starts with.
///
/// Repo rule, no source (G10). No wood source prints a helix pitch
/// (ruling Q8); the card says so.
pub const DRESSUP_HELIX_PITCH_MM: f64 = 1.0;

/// The helix entry radius as a fraction of the tool diameter, when the
/// operator has not set one: r = 0.3 x D.
///
/// Repo rule, no source (G10). No wood source prints a helix radius
/// (rulings Q6 and Q8). The dressup and the Adaptive3d entry both read it.
pub const HELIX_RADIUS_OVER_D: f64 = 0.3;

/// The diameter D (mm) that the helix rule [`HELIX_RADIUS_OVER_D`] and the
/// Adaptive3d `helix_radius_factor` scale: the tool's nominal cutting
/// diameter, `ToolConfig::diameter`, the one the operator enters.
///
/// For every cutter but a tapered ball this is `diameter()`. A tapered
/// ball's `diameter()` is its shaft (the envelope); a helix scaled by the
/// shaft runs past the tip and left a 3.65 mm centre pip on the 3.175 mm
/// matrix tool, so it reads the tip ball diameter (lead decision
/// 2026-09-25, G10 Part B). Every entry-radius reader goes through here.
#[must_use]
pub fn helix_entry_diameter_mm(cutter: &dyn crate::tool::MillingCutter) -> f64 {
    match cutter.geometry_hint() {
        crate::feeds::ToolGeometryHint::TaperedBall { tip_radius, .. } => 2.0 * tip_radius,
        _ => cutter.diameter(),
    }
}

/// A helix entry radius after the G10 rules (ruling Q6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HelixRadius {
    /// The radius asked for (mm): the operator value, or
    /// [`HELIX_RADIUS_OVER_D`] x D.
    pub requested_mm: f64,
    /// The radius the engine emits (mm): the request capped at the flat
    /// bottom.
    pub emitted_mm: f64,
    /// The flat bottom radius (mm) of the tool, `width_at_height(0.0)`.
    /// `None` when the tool has no flat bottom (ball, tapered ball, V-bit):
    /// then a helix leaves a centre pip, and nothing caps r.
    pub flat_bottom_mm: Option<f64>,
    /// True when the request is the rule, not an operator value.
    pub from_rule: bool,
}

impl HelixRadius {
    /// Resolve a requested helix radius against a tool.
    ///
    /// Ruling Q6 (geometry): a helix radius larger than the flat bottom
    /// leaves a post of stock at the centre (IMCO, Sandvik and Fusion print
    /// the same limit). On a tool with a flat bottom, r is capped there.
    #[must_use]
    pub fn resolve(
        requested_mm: f64,
        from_rule: bool,
        cutter: &dyn crate::tool::MillingCutter,
    ) -> Self {
        let flat = cutter.width_at_height(0.0);
        let flat_bottom_mm = (flat > 1e-9).then_some(flat);
        let emitted_mm = flat_bottom_mm.map_or(requested_mm, |cap| requested_mm.min(cap));
        Self {
            requested_mm,
            emitted_mm,
            flat_bottom_mm,
            from_rule,
        }
    }

    /// True when the flat bottom cut the request.
    #[must_use]
    pub fn capped(&self) -> bool {
        self.emitted_mm < self.requested_mm - 1e-9
    }
}

/// Default height (mm) above the material top where a helix or ramp entry
/// starts: the 0.5 mm the adaptive3d rapid floor keeps over its stock read.
pub fn default_entry_clearance_mm() -> f64 {
    crate::dressup::ENTRY_CONTACT_CLEARANCE
}

/// Default deviation budget (mm) for [`DressupConfig::segment_merge`].
fn default_segment_merge_tolerance() -> f64 {
    0.3
}

/// The dogbone dressup's parameters. `None` on [`DressupConfig`] means the
/// dressup is off (CUT-13).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DogboneParams {
    /// Corner angle (degrees) at or below which a dogbone is cut.
    pub angle: f64,
}

impl Default for DogboneParams {
    fn default() -> Self {
        Self { angle: 90.0 }
    }
}

/// The lead-in / lead-out dressup's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LeadParams {
    /// Lead arc radius (mm).
    pub radius: f64,
    /// F-040: Lead-in feed rate (mm/min). When `Some`, lead-in arc moves
    /// emitted by `apply_lead_in_out` use this rate (typically slower than
    /// cutting feed for a softer entry / cleaner dwell mark). When `None`,
    /// lead-in inherits the operation's primary `feed_rate` (pre-F-040
    /// behaviour). Default `None`.
    pub in_feed_rate: Option<f64>,
    /// F-040: Lead-out feed rate (mm/min). Same fallback semantics —
    /// typically faster than cutting feed (chip-clear on exit). Default `None`.
    pub out_feed_rate: Option<f64>,
}

impl Default for LeadParams {
    fn default() -> Self {
        Self {
            radius: 2.0,
            in_feed_rate: None,
            out_feed_rate: None,
        }
    }
}

/// The link-moves dressup's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LinkDressupParams {
    /// Longest gap (mm) the tool may stay down across.
    pub max_distance: f64,
    /// Feed rate (mm/min) of a link move.
    pub feed_rate: f64,
}

impl Default for LinkDressupParams {
    fn default() -> Self {
        Self {
            max_distance: 10.0,
            feed_rate: 500.0,
        }
    }
}

/// The arc-fitting dressup's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArcFitParams {
    /// Deviation budget (mm) between the fitted arc and the linear run.
    pub tolerance: f64,
}

impl Default for ArcFitParams {
    fn default() -> Self {
        Self { tolerance: 0.05 }
    }
}

/// The segment-merge dressup's parameters.
///
/// Phase 1 (accel-friendly toolpaths): merge dense runs of consecutive
/// same-feed linear cut moves whose interior points lie within `tolerance`
/// of the retained chord, so a low-acceleration controller can ramp to the
/// commanded feed instead of stalling on sub-millimetre segments. Runs after
/// arc-fitting.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SegmentMergeParams {
    /// Deviation budget (mm). Default 0.3. On an op with a finished edge
    /// the pipeline runs the merge at most at
    /// [`crate::polygon::FlattenPolicy::UNTOLERANCED_MM`] (P6.0b,
    /// `OperationTransformCapabilities::segment_merge_tolerance`).
    pub tolerance: f64,
}

impl Default for SegmentMergeParams {
    fn default() -> Self {
        Self {
            tolerance: default_segment_merge_tolerance(),
        }
    }
}

/// Configurable dressups applied after toolpath generation.
///
/// CUT-13: five dressups carry their parameters inside an `Option` instead
/// of pairing an enable `bool` with a value field that means nothing while
/// the bool is false. "arc_fitting = false with arc_tolerance = 0.05" is no
/// longer representable.
///
/// The wire form is unchanged. [`DressupConfigWire`] below carries the flat
/// bool-plus-value key layout every project file, every MCP
/// `set_dressup_field` key and the wire snapshot already use, and
/// `#[serde(from, into)]` converts between the two. Nothing outside this
/// file sees the wire struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "DressupConfigWire", into = "DressupConfigWire")]
pub struct DressupConfig {
    pub entry_style: DressupEntryStyle,
    pub ramp_angle: f64,
    /// The helix entry radius (mm). `None` is the rule
    /// [`HELIX_RADIUS_OVER_D`] x D, applied where the tool is known;
    /// `Some(r)` is an operator value (G10 D2). Either way the engine emits
    /// the radius capped at the flat bottom: [`Self::helix_radius_for`].
    pub helix_radius: Option<f64>,
    pub helix_pitch: f64,
    /// The height (mm) above the material top where a helix or ramp entry
    /// starts. The air above it is a straight move (operator ruling
    /// 2026-09-25: never helix air).
    pub entry_clearance_mm: f64,
    /// `Some` when the dogbone dressup runs.
    pub dogbone: Option<DogboneParams>,
    /// `Some` when the lead-in / lead-out dressup runs.
    pub lead_in_out: Option<LeadParams>,
    /// `Some` when the link-moves dressup runs.
    pub link_moves: Option<LinkDressupParams>,
    /// `Some` when the arc-fitting dressup runs.
    pub arc_fitting: Option<ArcFitParams>,
    /// `Some` when the segment-merge dressup runs.
    pub segment_merge: Option<SegmentMergeParams>,
    /// Feed-rate optimisation. This one keeps the flat bool-plus-value
    /// shape (CUT-13 holdout, F016 not folded): its values have no
    /// [`DressupFieldDef::owner`], so they stay settable and stored while
    /// the dressup is off.
    pub feed_optimization: bool,
    pub feed_max_rate: f64,
    pub feed_ramp_rate: f64,
    /// Rapid-order optimisation. A bool with no value field of its own, so
    /// there is no pair to fold.
    pub optimize_rapid_order: bool,
}

/// The serialized shape of [`DressupConfig`]: one enable `bool` beside each
/// value field, exactly as every project file and every MCP dressup key
/// already spell it.
///
/// This struct exists so CUT-13 can fix the in-memory type without moving a
/// single wire key. Read `DressupConfig`; nothing else should name this.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the wire struct mirrors the saved keys, and each bool is one saved dressup switch"
)]
struct DressupConfigWire {
    entry_style: DressupEntryStyle,
    ramp_angle: f64,
    /// G10 D2: absent (or null) is the rule 0.3 x D. A saved project's
    /// `helix_radius = 2.0` loads as the operator value `Some(2.0)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    helix_radius: Option<f64>,
    helix_pitch: f64,
    #[serde(default = "default_entry_clearance_mm")]
    entry_clearance_mm: f64,
    dogbone: bool,
    dogbone_angle: f64,
    lead_in_out: bool,
    lead_radius: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lead_in_feed_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lead_out_feed_rate: Option<f64>,
    link_moves: bool,
    link_max_distance: f64,
    link_feed_rate: f64,
    arc_fitting: bool,
    arc_tolerance: f64,
    #[serde(default)]
    segment_merge: bool,
    #[serde(default = "default_segment_merge_tolerance")]
    segment_merge_tolerance: f64,
    feed_optimization: bool,
    feed_max_rate: f64,
    feed_ramp_rate: f64,
    optimize_rapid_order: bool,
}

impl From<DressupConfigWire> for DressupConfig {
    fn from(w: DressupConfigWire) -> Self {
        Self {
            entry_style: w.entry_style,
            ramp_angle: w.ramp_angle,
            helix_radius: w.helix_radius,
            helix_pitch: w.helix_pitch,
            entry_clearance_mm: w.entry_clearance_mm,
            dogbone: w.dogbone.then_some(DogboneParams {
                angle: w.dogbone_angle,
            }),
            lead_in_out: w.lead_in_out.then_some(LeadParams {
                radius: w.lead_radius,
                in_feed_rate: w.lead_in_feed_rate,
                out_feed_rate: w.lead_out_feed_rate,
            }),
            link_moves: w.link_moves.then_some(LinkDressupParams {
                max_distance: w.link_max_distance,
                feed_rate: w.link_feed_rate,
            }),
            arc_fitting: w.arc_fitting.then_some(ArcFitParams {
                tolerance: w.arc_tolerance,
            }),
            segment_merge: w.segment_merge.then_some(SegmentMergeParams {
                tolerance: w.segment_merge_tolerance,
            }),
            feed_optimization: w.feed_optimization,
            feed_max_rate: w.feed_max_rate,
            feed_ramp_rate: w.feed_ramp_rate,
            optimize_rapid_order: w.optimize_rapid_order,
        }
    }
}

impl From<DressupConfig> for DressupConfigWire {
    fn from(c: DressupConfig) -> Self {
        let dogbone = c.dogbone.unwrap_or_default();
        let lead = c.lead_in_out.unwrap_or_default();
        let link = c.link_moves.unwrap_or_default();
        let arc = c.arc_fitting.unwrap_or_default();
        let merge = c.segment_merge.unwrap_or_default();
        Self {
            entry_style: c.entry_style,
            ramp_angle: c.ramp_angle,
            helix_radius: c.helix_radius,
            helix_pitch: c.helix_pitch,
            entry_clearance_mm: c.entry_clearance_mm,
            dogbone: c.dogbone.is_some(),
            dogbone_angle: dogbone.angle,
            lead_in_out: c.lead_in_out.is_some(),
            lead_radius: lead.radius,
            lead_in_feed_rate: lead.in_feed_rate,
            lead_out_feed_rate: lead.out_feed_rate,
            link_moves: c.link_moves.is_some(),
            link_max_distance: link.max_distance,
            link_feed_rate: link.feed_rate,
            arc_fitting: c.arc_fitting.is_some(),
            arc_tolerance: arc.tolerance,
            segment_merge: c.segment_merge.is_some(),
            segment_merge_tolerance: merge.tolerance,
            feed_optimization: c.feed_optimization,
            feed_max_rate: c.feed_max_rate,
            feed_ramp_rate: c.feed_ramp_rate,
            optimize_rapid_order: c.optimize_rapid_order,
        }
    }
}

/// One dressup a toolpath can turn on, named by its enable key.
///
/// F016: the enable keys used to be `&str` matches that ended in a wildcard
/// arm, so a dressup missing from one of them read as "off" and as "owns no
/// value". Every decision on a dressup is now an exhaustive match on this
/// enum, and a new dressup is a compile error until each decision names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DressupKey {
    EntryStyle,
    Dogbone,
    LeadInOut,
    LinkMoves,
    ArcFitting,
    SegmentMerge,
    FeedOptimization,
    OptimizeRapidOrder,
}

impl DressupKey {
    /// Every dressup, in the order the field table lists them.
    pub const ALL: [Self; 8] = crate::all_variants!(DressupKey:
        EntryStyle, Dogbone, LeadInOut, LinkMoves, ArcFitting, SegmentMerge, FeedOptimization,
        OptimizeRapidOrder,
    );

    /// The wire key that turns the dressup on: a `DressupConfig` field name.
    #[must_use]
    pub const fn as_key(self) -> &'static str {
        match self {
            Self::EntryStyle => "entry_style",
            Self::Dogbone => "dogbone",
            Self::LeadInOut => "lead_in_out",
            Self::LinkMoves => "link_moves",
            Self::ArcFitting => "arc_fitting",
            Self::SegmentMerge => "segment_merge",
            Self::FeedOptimization => "feed_optimization",
            Self::OptimizeRapidOrder => "optimize_rapid_order",
        }
    }
}

/// One published dressup field: the wire name and what the field does.
///
/// CMP-17: the MCP `set_dressup_config` description used to be a hand-written
/// prose list. It named 17 fields of 23 and it advertised a dial nothing read.
/// This table is the one place the vocabulary is written down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DressupFieldDef {
    /// The wire name, identical to the serde name of the struct field.
    pub name: &'static str,
    /// What the field does, in one line.
    pub description: &'static str,
    /// The dressup whose `Option` holds this value (CUT-13), or `None` for a
    /// field that lives on `DressupConfig` itself (an enable key, the entry
    /// values, the feed-optimisation values). `set_dressup_field` refuses a
    /// value whose owner is off: the round trip would drop it.
    /// `the_owner_column_matches_the_round_trip` holds this column against
    /// the wire form.
    pub owner: Option<DressupKey>,
}

impl DressupConfig {
    /// Every settable dressup field.
    ///
    /// `dressup_field_names_are_published` (this file) holds the table against
    /// the struct, and the `rs_cam_viz` test of the same name holds the MCP
    /// `set_dressup_config` description against the table. A field added to
    /// `DressupConfig` without a row here fails the first test; a row that the
    /// description does not name fails the second.
    pub const FIELD_DEFS: &'static [DressupFieldDef] = &[
        DressupFieldDef {
            name: "entry_style",
            description: "how the tool enters the cut: none, ramp or helix",
            owner: None,
        },
        DressupFieldDef {
            name: "ramp_angle",
            description: "ramp entry angle (degrees)",
            owner: None,
        },
        DressupFieldDef {
            name: "helix_radius",
            description: "helix entry radius (mm); null = 0.3 x tool diameter (repo rule), capped at the flat bottom",
            owner: None,
        },
        DressupFieldDef {
            name: "helix_pitch",
            description: "helix entry pitch per turn (mm)",
            owner: None,
        },
        DressupFieldDef {
            name: "entry_clearance_mm",
            description: "height above the material where a helix or ramp entry starts (mm)",
            owner: None,
        },
        DressupFieldDef {
            name: "dogbone",
            description: "add dogbone overcuts at inside corners",
            owner: None,
        },
        DressupFieldDef {
            name: "dogbone_angle",
            description: "corner angle at or below which a dogbone is cut (degrees)",
            owner: Some(DressupKey::Dogbone),
        },
        DressupFieldDef {
            name: "lead_in_out",
            description: "add tangential lead-in and lead-out arcs",
            owner: None,
        },
        DressupFieldDef {
            name: "lead_radius",
            description: "lead-in and lead-out arc radius (mm)",
            owner: Some(DressupKey::LeadInOut),
        },
        DressupFieldDef {
            name: "lead_in_feed_rate",
            description: "feed for the lead-in arc (mm/min); unset inherits the cutting feed",
            owner: Some(DressupKey::LeadInOut),
        },
        DressupFieldDef {
            name: "lead_out_feed_rate",
            description: "feed for the lead-out arc (mm/min); unset inherits the cutting feed",
            owner: Some(DressupKey::LeadInOut),
        },
        DressupFieldDef {
            name: "link_moves",
            description: "keep the tool down between near passes instead of retracting",
            owner: None,
        },
        DressupFieldDef {
            name: "link_max_distance",
            description: "longest gap a link move may span (mm)",
            owner: Some(DressupKey::LinkMoves),
        },
        DressupFieldDef {
            name: "link_feed_rate",
            description: "feed for a link move (mm/min)",
            owner: Some(DressupKey::LinkMoves),
        },
        DressupFieldDef {
            name: "arc_fitting",
            description: "fit linear runs into G2 and G3 arcs",
            owner: None,
        },
        DressupFieldDef {
            name: "arc_tolerance",
            description: "deviation budget for arc fitting (mm)",
            owner: Some(DressupKey::ArcFitting),
        },
        DressupFieldDef {
            name: "segment_merge",
            description: "merge dense cut runs into longer moves; default on for roughing; a 3D Rough planner applies it before it records each cut",
            owner: None,
        },
        DressupFieldDef {
            name: "segment_merge_tolerance",
            description: "deviation budget for segment merge (mm)",
            owner: Some(DressupKey::SegmentMerge),
        },
        DressupFieldDef {
            name: "feed_optimization",
            description: "scale the feed with stock engagement",
            owner: None,
        },
        DressupFieldDef {
            name: "feed_max_rate",
            description: "upper feed the optimiser may command (mm/min)",
            owner: None,
        },
        DressupFieldDef {
            name: "feed_ramp_rate",
            description: "how fast the optimiser may change the feed (mm/min per mm)",
            owner: None,
        },
        DressupFieldDef {
            name: "optimize_rapid_order",
            description: "reorder disconnected fragments to cut rapid travel",
            owner: None,
        },
    ];

    /// The published field names, in table order.
    #[must_use]
    pub fn published_field_names() -> Vec<&'static str> {
        Self::FIELD_DEFS.iter().map(|d| d.name).collect()
    }
}

impl Default for DressupConfig {
    fn default() -> Self {
        // Roadmap B.6 — three pure-flips: link_moves, feed_optimization,
        // optimize_rapid_order. All are wins for the typical case;
        // operations that can't tolerate them are stripped by
        // `normalize_for_op` (which `for_op` now also calls on construct).
        Self {
            entry_style: DressupEntryStyle::None,
            ramp_angle: DRESSUP_RAMP_ANGLE_DEG,
            helix_radius: None,
            helix_pitch: DRESSUP_HELIX_PITCH_MM,
            entry_clearance_mm: default_entry_clearance_mm(),
            dogbone: None,
            lead_in_out: None,
            link_moves: Some(LinkDressupParams::default()),
            arc_fitting: None,
            segment_merge: None,
            feed_optimization: true,
            feed_max_rate: 3000.0,
            feed_ramp_rate: 200.0,
            optimize_rapid_order: true,
        }
    }
}

impl DressupConfig {
    /// The dressup whose `Option` holds a value key, read from the
    /// [`DressupFieldDef::owner`] column. `None` for an unknown key and for a
    /// field that lives on `DressupConfig` itself.
    ///
    /// `set_dressup_field` uses this to refuse a value patch whose dressup is
    /// off: with `Option<Params>` that value has nowhere to live, so writing
    /// it would be a set the caller never gets back.
    #[must_use]
    pub fn owner_of_value_field(key: &str) -> Option<DressupKey> {
        Self::FIELD_DEFS
            .iter()
            .find(|def| def.name == key)
            .and_then(|def| def.owner)
    }

    /// The helix radius this config emits on `cutter`: the operator value,
    /// or [`HELIX_RADIUS_OVER_D`] x D, capped at the flat bottom (G10 Q6).
    #[must_use]
    pub fn helix_radius_for(&self, cutter: &dyn crate::tool::MillingCutter) -> HelixRadius {
        match self.helix_radius {
            Some(r) => HelixRadius::resolve(r, false, cutter),
            None => HelixRadius::resolve(
                HELIX_RADIUS_OVER_D * helix_entry_diameter_mm(cutter),
                true,
                cutter,
            ),
        }
    }

    /// Whether the dressup is on. An entry style of `None` is off.
    #[must_use]
    pub fn is_dressup_enabled(&self, key: DressupKey) -> bool {
        match key {
            DressupKey::EntryStyle => self.entry_style != DressupEntryStyle::None,
            DressupKey::Dogbone => self.dogbone.is_some(),
            DressupKey::LeadInOut => self.lead_in_out.is_some(),
            DressupKey::LinkMoves => self.link_moves.is_some(),
            DressupKey::ArcFitting => self.arc_fitting.is_some(),
            DressupKey::SegmentMerge => self.segment_merge.is_some(),
            DressupKey::FeedOptimization => self.feed_optimization,
            DressupKey::OptimizeRapidOrder => self.optimize_rapid_order,
        }
    }

    /// Smart defaults based on operation process role.
    pub fn for_role(role: super::catalog::UiProcessRole) -> Self {
        use super::catalog::UiProcessRole;
        let base = Self::default();
        match role {
            // Roadmap B.5 — Roughing operations get a Ramp entry by
            // default (was None, which gave every Pocket / Profile /
            // Adaptive / Face / Adaptive3d a vertical plunge). `for_op`
            // starts Adaptive with Helix (G10 D3) and strips back to None
            // for Drill/Trace.
            UiProcessRole::Roughing => Self {
                entry_style: DressupEntryStyle::Ramp,
                arc_fitting: Some(ArcFitParams::default()),
                // Phase 1: the merge helps a controller track the feed.
                // P6.0b: the premise "roughing leaves >= 0.5 mm stock" is
                // false for Pocket, Profile and Zigzag, which cut the
                // finished wall. The pipeline caps the merge on an op with
                // a finished edge (`OperationTransformCapabilities::
                // segment_merge_tolerance`). Finish/SemiFinish stay off.
                segment_merge: Some(SegmentMergeParams::default()),
                link_moves: Some(LinkDressupParams::default()),
                optimize_rapid_order: true,
                ..base
            },
            // R10 (2026-09-18): the finishing families get NO entry ramp
            // by default (was Ramp). A finishing contour starts on a
            // 0.5 mm leave, so a plunge there is a 0.5 mm bite. A 3 degree
            // ramp is 19 mm of XY per mm of descent, and on a short run the
            // fold lays that XY as laps over the run: the Corne waterline
            // sawed a 3 mm wall to pins along a 2 mm run (case analysis
            // §4.5). The fold's lap cap refuses the saw; this default stops
            // the finishing families asking for it. Roughing keeps Ramp.
            UiProcessRole::SemiFinish => Self {
                entry_style: DressupEntryStyle::None,
                arc_fitting: Some(ArcFitParams::default()),
                optimize_rapid_order: true,
                ..base
            },
            UiProcessRole::Finish => Self {
                entry_style: DressupEntryStyle::None,
                lead_in_out: Some(LeadParams::default()),
                arc_fitting: Some(ArcFitParams::default()),
                optimize_rapid_order: true,
                ..base
            },
        }
    }

    /// Smart defaults based on the operation type: the role-level base,
    /// then the registry's own per-op policy. No op is named here.
    pub fn for_op(op: super::catalog::OperationType) -> Self {
        use super::catalog::EntryStylePolicy;
        let mut cfg = Self::for_role(op.spec().ui_process_role);
        // G10 D3 (operator decision 2026-09-25): a `DefaultHelix` op starts
        // a NEW toolpath with Helix. This is the only reader of the policy;
        // `normalize_for_op` never rewrites the operator's style afterwards.
        if op.registry_entry().dressup_policy.entry == EntryStylePolicy::DefaultHelix {
            cfg.entry_style = DressupEntryStyle::Helix;
        }
        // CMP-26: the hand-written ProjectCurve strip that stood here was
        // a no-op. `normalize_for_op` below reads
        // `op.registry_entry().dressup_policy`, and ProjectCurve's row is
        // `DressupPolicy::strip_all("Incompatible with Project Curve: each
        // ring would get a phantom diagonal cut.")`, which sets exactly
        // those three fields — with the user-facing reason the GUI greys
        // the controls with. This was the last op-specific arm in a method
        // whose doc says the decisions moved to the registry in Phase 1 T5.
        //
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
            if self.lead_in_out.is_some() {
                self.lead_in_out = None;
                changed = true;
            }
            if self.link_moves.is_some() {
                self.link_moves = None;
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
            // G10 D3: `DefaultHelix` is a construction default, read by
            // `for_op` only. An operator Ramp stays Ramp.
            EntryStylePolicy::DefaultHelix | EntryStylePolicy::AnyEntry => {}
        }
        changed
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    /// CUT-13 — the wire form carries every dressup value field.
    ///
    /// The in-memory type keeps each dressup's parameters inside an `Option`;
    /// the serialized form keeps the flat enable-bool-beside-value-field
    /// layout every project file and every MCP `set_dressup_field` key uses.
    /// `DressupConfigWire` is the only thing between them, and a value it
    /// forgets is a dial an operator sets and never gets back.
    ///
    /// This test sets a NON-DEFAULT value on all five folded dressups, so a
    /// conversion that substitutes the module default fails here. The
    /// round-trip sentries alone cannot see that: their fixtures carry the
    /// defaults.
    #[test]
    fn the_wire_form_carries_every_dressup_value_field() {
        let cfg = DressupConfig {
            dogbone: Some(DogboneParams { angle: 111.0 }),
            lead_in_out: Some(LeadParams {
                radius: 3.5,
                in_feed_rate: Some(321.0),
                out_feed_rate: Some(654.0),
            }),
            link_moves: Some(LinkDressupParams {
                max_distance: 17.0,
                feed_rate: 432.0,
            }),
            arc_fitting: Some(ArcFitParams { tolerance: 0.017 }),
            segment_merge: Some(SegmentMergeParams { tolerance: 0.19 }),
            ..DressupConfig::default()
        };
        let json = serde_json::to_value(&cfg).unwrap();
        // The flat keys are what a project file and an MCP client write.
        for (key, want) in [
            ("dogbone_angle", 111.0),
            ("lead_radius", 3.5),
            ("lead_in_feed_rate", 321.0),
            ("lead_out_feed_rate", 654.0),
            ("link_max_distance", 17.0),
            ("link_feed_rate", 432.0),
            ("arc_tolerance", 0.017),
            ("segment_merge_tolerance", 0.19),
        ] {
            let got = json[key].as_f64().unwrap_or_else(|| {
                panic!("the wire form must carry '{key}'");
            });
            assert!((got - want).abs() < 1e-12, "{key}: {got} is not {want}");
        }
        for key in [
            "dogbone",
            "lead_in_out",
            "link_moves",
            "arc_fitting",
            "segment_merge",
        ] {
            assert_eq!(json[key], serde_json::json!(true), "{key} must read on");
        }

        let back: DressupConfig = serde_json::from_value(json).unwrap();
        assert_eq!(back.dogbone, cfg.dogbone);
        assert_eq!(back.lead_in_out, cfg.lead_in_out);
        assert_eq!(back.link_moves, cfg.link_moves);
        assert_eq!(back.arc_fitting, cfg.arc_fitting);
        assert_eq!(back.segment_merge, cfg.segment_merge);
    }

    /// F016 — the owner column says where each value lives.
    ///
    /// Start from a config with every `Option` dressup off. Write a new
    /// value to one field through the wire form and read it back. A field
    /// with an owner lives inside that dressup's `Option`, so the round trip
    /// drops it; a field with no owner lives on the config and keeps it. A
    /// value field added inside an `Option` without an owner fails here,
    /// which is the F016 defect: the setter accepted the value and the round
    /// trip dropped it.
    #[test]
    fn the_owner_column_matches_the_round_trip() {
        let all_off = DressupConfig {
            entry_style: DressupEntryStyle::None,
            dogbone: None,
            lead_in_out: None,
            link_moves: None,
            arc_fitting: None,
            segment_merge: None,
            feed_optimization: false,
            optimize_rapid_order: false,
            ..DressupConfig::default()
        };
        let base = serde_json::to_value(&all_off).unwrap();
        let mut owned = 0;
        for def in DressupConfig::FIELD_DEFS {
            let new_value = match base.get(def.name) {
                Some(serde_json::Value::Bool(b)) => serde_json::json!(!b),
                Some(serde_json::Value::String(_)) => serde_json::json!("ramp"),
                _ => serde_json::json!(1234.5),
            };
            let mut patched = base.clone();
            patched[def.name] = new_value.clone();
            let back: DressupConfig = serde_json::from_value(patched).unwrap();
            let json = serde_json::to_value(&back).unwrap();
            let kept = json.get(def.name) == Some(&new_value);
            match def.owner {
                Some(owner) => {
                    owned += 1;
                    assert!(
                        !kept,
                        "'{}' names owner '{}' but the round trip keeps it while the owner is off",
                        def.name,
                        owner.as_key()
                    );
                    assert!(
                        DressupConfig::FIELD_DEFS
                            .iter()
                            .any(|d| d.name == owner.as_key()),
                        "the owner '{}' is not a published field",
                        owner.as_key()
                    );
                }
                None => assert!(
                    kept,
                    "'{}' has no owner but the round trip drops it while every dressup is off",
                    def.name
                ),
            }
        }
        assert_eq!(owned, 8, "the table names {owned} owned value fields");
        for key in DressupKey::ALL {
            assert!(
                DressupConfig::FIELD_DEFS
                    .iter()
                    .any(|d| d.name == key.as_key()),
                "the enable key '{}' is not a published field",
                key.as_key()
            );
        }
    }

    /// A dressup that is off still writes its flat keys, with the module
    /// default beside a `false`. That is what every project file written
    /// before CUT-13 holds, so the reader must keep producing it.
    #[test]
    fn a_dressup_that_is_off_still_writes_its_flat_keys() {
        let json = serde_json::to_value(DressupConfig::default()).unwrap();
        assert_eq!(json["arc_fitting"], serde_json::json!(false));
        assert!((json["arc_tolerance"].as_f64().unwrap() - 0.05).abs() < 1e-12);
        assert_eq!(json["dogbone"], serde_json::json!(false));
        assert!((json["dogbone_angle"].as_f64().unwrap() - 90.0).abs() < 1e-12);

        // And a file that carries a value beside a `false` loads with the
        // dressup off; the orphan value is what CUT-13 stopped representing.
        let mut obj = json;
        obj["arc_tolerance"] = serde_json::json!(0.3);
        let cfg: DressupConfig = serde_json::from_value(obj).unwrap();
        assert!(cfg.arc_fitting.is_none());
    }

    /// CMP-17 — the published dressup table names every `DressupConfig` field.
    ///
    /// The names are read from a serialized instance, not from a hand-written
    /// list, so a field added to the struct shows up here. Both `Option`
    /// fields carry `skip_serializing_if = "Option::is_none"`, so the instance
    /// sets them to `Some`; a default instance would hide them and the test
    /// would pass while the table was wrong in exactly the direction CMP-17
    /// reports.
    ///
    /// NOT MEASURED: the description text of a row, and whether any code reads
    /// the field. The `rs_cam_viz` test of the same name holds the MCP
    /// description against this table.
    #[test]
    fn dressup_field_names_are_published() {
        let full = DressupConfig {
            lead_in_out: Some(LeadParams {
                radius: 2.0,
                in_feed_rate: Some(300.0),
                out_feed_rate: Some(900.0),
            }),
            // G10 D2: the rule (`None`) is not written, so an operator value
            // puts the key on the wire.
            helix_radius: Some(2.0),
            ..DressupConfig::default()
        };
        let value = serde_json::to_value(&full).unwrap();
        let serialized: std::collections::BTreeSet<String> = value
            .as_object()
            .expect("DressupConfig serializes to an object")
            .keys()
            .cloned()
            .collect();
        let published: std::collections::BTreeSet<String> = DressupConfig::published_field_names()
            .into_iter()
            .map(str::to_owned)
            .collect();

        let missing: Vec<&String> = serialized.difference(&published).collect();
        assert!(
            missing.is_empty(),
            "DressupConfig fields absent from DressupConfig::FIELD_DEFS: {missing:?}"
        );
        let extra: Vec<&String> = published.difference(&serialized).collect();
        assert!(
            extra.is_empty(),
            "DressupConfig::FIELD_DEFS rows that name no field: {extra:?}"
        );
    }

    fn test_ctx() -> HeightContext {
        HeightContext {
            safe_z: 10.0,
            op_depth: 5.0,
            stock_top_z: 25.0,
            stock_bottom_z: 0.0,
            model_top_z: Some(20.0),
            model_bottom_z: Some(2.0),
            floor: None,
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
                // Inlay and InlayPlug joined 2026-10-14 (W1.4,
                // G-INLAYSPLIT) and VCarve the same day (W1.4b,
                // G-VCARVELEAD): each run starts on the design outline at
                // zero depth, and a lead there cuts outside the design.
                // Bore joined 2026-10-22 (P6.6): it cuts its own helix
                // entry, and a lead or a link has no room in the hole.
                OperationType::ProjectCurve
                | OperationType::DropCutter
                | OperationType::UnifiedFinish
                | OperationType::Inlay
                | OperationType::InlayPlug
                | OperationType::VCarve
                | OperationType::Bore => {
                    assert!(policy.strip_all_reason.is_some(), "{op:?}: strip-all");
                    assert_eq!(policy.entry, EntryStylePolicy::AnyEntry);
                }
                // AlignmentPinDrill joined the force-no-entry set 2026-08-21
                // (G-WANAKA-DRILL-RAMP): it had been ANY_DRESSUP while its
                // sibling Drill was ForceNone, so `DressupConfig::for_op`
                // shipped a role-default Ramp on the one op whose entire
                // purpose is flip registration. A ramped pin hole is an oval
                // slot — the emitted motion on a real job was a 19 mm lateral
                // move while descending. The generator hard-strips it, so the
                // policy row was the last place config and behaviour still
                // disagreed.
                OperationType::Drill
                | OperationType::AlignmentPinDrill
                | OperationType::Trace
                | OperationType::Adaptive3d => {
                    assert!(policy.strip_all_reason.is_none(), "{op:?}");
                    assert_eq!(policy.entry, EntryStylePolicy::ForceNone, "{op:?}");
                }
                OperationType::Adaptive => {
                    assert!(policy.strip_all_reason.is_none());
                    assert_eq!(policy.entry, EntryStylePolicy::DefaultHelix);
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

    /// G10 D3: a new 2D Adaptive toolpath starts with Helix, and
    /// `normalize_for_op` keeps an operator Ramp (it used to upgrade it).
    #[test]
    fn a_new_adaptive_is_helix_and_an_operator_ramp_stays_ramp() {
        use super::super::catalog::OperationType;
        let fresh = DressupConfig::for_op(OperationType::Adaptive);
        assert_eq!(fresh.entry_style, DressupEntryStyle::Helix);
        let mut ramp = DressupConfig {
            entry_style: DressupEntryStyle::Ramp,
            ..fresh
        };
        assert!(!ramp.normalize_for_op(OperationType::Adaptive));
        assert_eq!(ramp.entry_style, DressupEntryStyle::Ramp);
        // A Pocket keeps the role default Ramp.
        assert_eq!(
            DressupConfig::for_op(OperationType::Pocket).entry_style,
            DressupEntryStyle::Ramp
        );
    }

    /// G10 D2: `helix_radius` is `None` (the rule) by default and is not
    /// written; a saved `2.0` loads as the operator value `Some(2.0)`.
    #[test]
    fn a_saved_helix_radius_loads_as_an_operator_value() {
        let default = DressupConfig::default();
        assert_eq!(default.helix_radius, None);
        let mut json = serde_json::to_value(&default).unwrap();
        assert!(
            json.get("helix_radius").is_none(),
            "the rule is not written"
        );
        json["helix_radius"] = serde_json::json!(2.0);
        let loaded: DressupConfig = serde_json::from_value(json).unwrap();
        assert_eq!(loaded.helix_radius, Some(2.0));
        let back = serde_json::to_value(&loaded).unwrap();
        assert_eq!(back["helix_radius"].as_f64(), Some(2.0));
    }

    /// G10 Q6: the rule is 0.3 x D; a radius above the flat bottom is
    /// capped there; a tool with no flat bottom is not capped.
    #[test]
    fn the_helix_radius_is_the_rule_capped_at_the_flat_bottom() {
        use crate::tool::{BallEndmill, FlatEndmill};
        let flat = FlatEndmill::new(3.175, 20.0);
        let rule = DressupConfig::default().helix_radius_for(&flat);
        assert!(rule.from_rule && !rule.capped());
        assert!((rule.emitted_mm - HELIX_RADIUS_OVER_D * 3.175).abs() < 1e-12);
        let op = DressupConfig {
            helix_radius: Some(2.0),
            ..DressupConfig::default()
        };
        let capped = op.helix_radius_for(&flat);
        assert!(!capped.from_rule && capped.capped());
        assert!((capped.emitted_mm - 3.175 / 2.0).abs() < 1e-12);
        let ball = BallEndmill::new(6.0, 20.0);
        let free = op.helix_radius_for(&ball);
        assert_eq!(free.flat_bottom_mm, None);
        assert!((free.emitted_mm - 2.0).abs() < 1e-12);
    }

    /// CMP-16: the config's rest defaults ARE the detector's, not a copy
    /// of them.
    ///
    /// The struct doc said the three numbers "should stay numerically in
    /// sync", both files wrote them as separate literals, and nothing
    /// compared the two. The defaults are derived now, so this test is a
    /// guard against someone re-inlining a literal — which is a real
    /// possibility, not a tautology: the derive is three field reads that
    /// an edit can replace one at a time.
    #[test]
    fn rest_analysis_config_defaults_match_the_detector() {
        let cfg = RestAnalysisConfig::default();
        let detector = crate::surface::rest_field::RestFieldParams::default();

        assert!((cfg.cell_mm - detector.cell_mm).abs() < f64::EPSILON);
        assert!((cfg.min_valley_depth - detector.min_valley_depth).abs() < f64::EPSILON);
        assert!((cfg.region_margin_mm - detector.region_margin_mm).abs() < f64::EPSILON);

        // Non-vacuity: a zeroed detector default would satisfy the three
        // comparisons above and mean nothing.
        assert!(detector.cell_mm > 0.0);
        assert!(detector.min_valley_depth > 0.0);
        assert!(detector.region_margin_mm > 0.0);

        // The two `None` fields are NOT the detector's numbers. Both are
        // instructions to ask a policy at use time, and reading a literal
        // from the detector would freeze the answer — see the field docs
        // (PR-7 / H2.5).
        assert_eq!(cfg.offset_stepover_mm, None);
        assert_eq!(cfg.num_offset_passes, None);
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
            lead_in_out: Some(LeadParams::default()),
            link_moves: Some(LinkDressupParams::default()),
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
            assert!(cfg.lead_in_out.is_none(), "{op:?}");
            assert!(cfg.link_moves.is_none(), "{op:?}");
            // Idempotent.
            assert!(!cfg.normalize_for_op(op));
        }

        // CMP-26: `for_op` — not just `normalize_for_op` — must produce
        // the strip from the REGISTRY alone. `for_op` used to hand-write
        // the ProjectCurve strip four lines before calling
        // `normalize_for_op`, with its own rationale comment beside the
        // registry's own. Added BEFORE that block was deleted, so it
        // pins the resulting config across the delete rather than after
        // it.
        for op in [
            OperationType::ProjectCurve,
            OperationType::DropCutter,
            OperationType::UnifiedFinish,
        ] {
            let cfg = DressupConfig::for_op(op);
            assert_eq!(cfg.entry_style, DressupEntryStyle::None, "{op:?}");
            assert!(cfg.lead_in_out.is_none(), "{op:?}");
            assert!(cfg.link_moves.is_none(), "{op:?}");
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
            assert!(
                cfg.lead_in_out.is_some(),
                "{op:?}: lead-in/out must survive"
            );
            assert!(cfg.link_moves.is_some(), "{op:?}: link moves must survive");
        }

        // Default-helix (G10 D3): a construction default only. An operator
        // Ramp, a Helix and None all pass through.
        let mut cfg = dirty();
        assert!(!cfg.normalize_for_op(OperationType::Adaptive));
        assert_eq!(cfg.entry_style, DressupEntryStyle::Ramp);
        cfg.entry_style = DressupEntryStyle::Helix;
        assert!(!cfg.normalize_for_op(OperationType::Adaptive));
        cfg.entry_style = DressupEntryStyle::None;
        assert!(!cfg.normalize_for_op(OperationType::Adaptive));

        // Unrestricted op: nothing changes.
        let mut cfg = dirty();
        assert!(!cfg.normalize_for_op(OperationType::Pocket));
        assert_eq!(cfg.entry_style, DressupEntryStyle::Ramp);
        assert!(cfg.lead_in_out.is_some() && cfg.link_moves.is_some());
    }

    /// F033: THE order rule. Each kind fires on its own plane pair, the
    /// top is anchored on the stock top for a `top_on_stock_box` op (R7),
    /// and the bottom counts only where it reaches the motion.
    #[test]
    fn height_order_names_each_kind_with_its_anchor_f033() {
        use crate::compute::catalog::OperationType;
        let in_order = ResolvedHeights {
            clearance_z: 30.0,
            retract_z: 20.0,
            feed_z: 18.0,
            top_z: 10.0,
            bottom_z: 0.0,
            top_pinned: false,
            bottom_pinned: false,
        };
        assert!(height_order(OperationType::Waterline, &in_order, Some(10.0)).is_empty());

        let kinds = |h: &ResolvedHeights, op: OperationType, stock: Option<f64>| {
            height_order(op, h, stock)
                .into_iter()
                .map(|v| std::mem::discriminant(&v))
                .collect::<Vec<_>>()
        };
        let d = |v: HeightOrderViolation| std::mem::discriminant(&v);
        let [clear, retract_top, retract_feed, feed_top, bottom] = HeightOrderViolation::EXAMPLES;

        let h = ResolvedHeights {
            clearance_z: 15.0,
            ..in_order
        };
        assert_eq!(
            kinds(&h, OperationType::Waterline, Some(10.0)),
            vec![d(clear)]
        );
        let h = ResolvedHeights {
            retract_z: 17.0,
            ..in_order
        };
        assert_eq!(
            kinds(&h, OperationType::Waterline, Some(10.0)),
            vec![d(retract_feed)]
        );
        let h = ResolvedHeights {
            feed_z: 9.0,
            ..in_order
        };
        assert_eq!(
            kinds(&h, OperationType::Waterline, Some(10.0)),
            vec![d(feed_top)]
        );
        let h = ResolvedHeights {
            retract_z: 9.0,
            feed_z: 8.0,
            ..in_order
        };
        assert_eq!(
            kinds(&h, OperationType::Waterline, Some(10.0)),
            vec![d(retract_top), d(feed_top)]
        );
        // `top > bottom` is strict for a ladder that excludes its top: an
        // equal bottom is an empty ladder.
        let h = ResolvedHeights {
            bottom_z: 10.0,
            ..in_order
        };
        assert_eq!(
            kinds(&h, OperationType::Pocket, Some(10.0)),
            vec![d(bottom)]
        );
        // F033b: a ladder that includes its top (`ladder_includes_top`)
        // cuts one level at bottom = top, so it is no break.
        assert!(kinds(&h, OperationType::Waterline, Some(10.0)).is_empty());
        // An `OpDial` op does not read the bottom (F1.19).
        assert!(kinds(&h, OperationType::DropCutter, Some(10.0)).is_empty());
        // R7: Adaptive 3D starts on the stock top, so a pinned top below a
        // pinned bottom is not a break when the bottom is under the stock top.
        let h = ResolvedHeights {
            top_z: 0.1,
            bottom_z: 4.0,
            ..in_order
        };
        assert!(kinds(&h, OperationType::Adaptive3d, Some(10.0)).is_empty());
        assert_eq!(
            kinds(&h, OperationType::Waterline, Some(10.0)),
            vec![d(bottom)]
        );

        // One severity per kind.
        assert!(clear.refuses() && retract_top.refuses() && bottom.refuses());
        assert!(!retract_feed.refuses() && !feed_top.refuses());
        // An edit door refuses only an ADDED refusing kind.
        assert_eq!(added_refusals(&[], &[clear, feed_top]), vec![clear]);
        assert!(added_refusals(&[clear], &[clear]).is_empty());
    }
}
