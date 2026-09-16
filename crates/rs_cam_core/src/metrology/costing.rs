//! The candidate costing harness: production relink, then the F-034
//! integrator.
//!
//! Promoted verbatim from `tests/thin_organic_island_widths.rs`
//! (`relink_and_cost_under`, the most complete of the six instrument
//! copies). Candidate constructors hand it raw toolpaths; it applies the
//! SAME production relink to every arm and then prices the linked motion
//! with `crate::machine::kinematics::compute_cycle_time`.
//!
//! # The two time scales (measurement contract, rule 3)
//!
//! This harness prices ONE candidate toolpath under test-pinned feeds. The
//! session integrator prices a whole simulated project. The integrator
//! arithmetic is shared; the scope is not. A table that mixes the two
//! scales must label the scale of every row.
//!
//! # The six instrument copies, and where they diverged
//!
//! `relink_and_cost` existed in six test files. Five were byte-equivalent
//! up to monomorphized cutter type and which [`CandidateCost`] fields they
//! kept. One diverged for real:
//!
//! * `monotone_cell_decomposition_c2.rs` passed `link_kinematics: None`,
//!   so the relink kept ANY gouge-safe link instead of costing each link
//!   against the retract it replaces. That is the [`CostingContext::
//!   kinematics`]` = None` arm here, preserved as an explicit parameter.
//!
//! The shared relink parameters are the production ones and identical in
//! every copy: `hookup_distance` 25.0 (the operator's
//! `intra_region_hookup_mm`, `wanaka200_mt2.toml:949`), `sampling` 0.5,
//! `stock_to_leave` 0.0, `reorder: true`, and the region's own polygon as
//! boundary. Feeds were per-instrument constants and ride
//! [`CostingFeeds`].

use crate::geometry::region_set::RegionSet;
use crate::machine::kinematics::{LinkKinematics, MachineKinematics, compute_cycle_time};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::surface_link::LinkCeiling;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

/// Feed-rate pins the harness prices a candidate under. Every instrument
/// states its own; the wanaka-tier instruments share
/// `feed 735 / plunge 180 / max 10_000 / rapid 5_000`.
#[derive(Debug, Clone, Copy)]
pub struct CostingFeeds {
    /// Cutting feed written onto relink surface links (mm/min).
    pub feed_mm_min: f64,
    /// Plunge feed written onto relink plunges (mm/min).
    pub plunge_mm_min: f64,
    /// Machine feed ceiling the integrator clamps to (mm/min).
    pub max_feed_mm_min: f64,
    /// Rapid traverse rate the integrator prices rapids at (mm/min).
    pub rapid_feed_mm_min: f64,
}

/// Everything a costing call shares across arms: the surface, the cutter,
/// the machine, and the feed pins. Built once per comparison so every arm
/// is priced on the same terms.
pub struct CostingContext<'a> {
    pub mesh: &'a TriangleMesh,
    pub index: &'a SpatialIndex,
    pub cutter: &'a dyn MillingCutter,
    /// `Some` prices each candidate link against the retract it replaces
    /// (the F-034 link/retract cost gate) and integrates `time_s`. `None`
    /// keeps any gouge-safe link and reports `time_s` as `NaN` — the
    /// `monotone_cell_decomposition_c2.rs` divergence, preserved
    /// explicitly.
    pub kinematics: Option<&'a MachineKinematics>,
    pub feeds: CostingFeeds,
}

/// One costed arm.
///
/// The union of the fields the six instrument copies kept. Every field is
/// computed on every call; an instrument reads the columns it registered.
pub struct CandidateCost {
    pub moves: usize,
    pub cutting_mm: f64,
    pub rapid_mm: f64,
    /// Integrated cycle time (s). `NaN` when [`CostingContext::kinematics`]
    /// is `None` — not measured, never zero.
    pub time_s: f64,
    pub fragments: usize,
    pub linked: usize,
    pub kept_retracts: usize,
    /// Junctions the F-034 link/retract cost gate declined because the link
    /// was gouge-safe but SLOWER than the retract it would replace. Under a
    /// ceiling every kept link grows two vertical legs, so this is the
    /// channel through which ceiling HEIGHT turns into retracts.
    pub slower_than_retract: usize,
    /// Junctions refused outright because the ceiling reached `safe_z`.
    /// **Structurally `0`** for any arm with `link_ceiling: None`
    /// (`surface_link.rs`), so a zero here is only evidence when a ceiling
    /// was actually in scope.
    pub ceiling_above_safe_z: usize,
    /// The relinked motion itself, for instruments that render or replay it.
    pub path: Toolpath,
}

/// Which LINK REGIME an arm is costed in.
///
/// [`relink_and_cost`] keeps the fresh-stock behaviour; this exists so an
/// instrument can cost the SAME candidate under the ceiling the live rest
/// op passes, through the same relink site. `safe_z` rides here rather
/// than as an extra parameter of the kernel.
#[derive(Clone, Copy)]
pub struct LinkRegime<'a> {
    pub label: &'static str,
    pub safe_z: f64,
    pub ceiling: Option<LinkCeiling<'a>>,
    pub flush_ride: bool,
    pub airborne: bool,
}

impl<'a> LinkRegime<'a> {
    /// The arm the thin-organic FINDINGS §0d–§0h numbers were measured in:
    /// no ceiling, so a link rides the mesh surface directly.
    ///
    /// `flush_ride` and `airborne_links_may_leave_territory` are `false`
    /// here where production's finishing site sets both `true`
    /// (`unified_finish.rs`) — and that is **not** a divergence: both flags
    /// are inert without a ceiling. `flush_ride` is documented "Ignored
    /// when `link_ceiling` is `None`", and the airborne exemption is
    /// conjunctive with the link's shape, so a surface-riding link "stays
    /// vetoed everywhere, whatever this flag says" (`surface_link.rs`).
    #[must_use]
    pub const fn fresh_stock(safe_z: f64) -> Self {
        Self {
            label: "fresh",
            safe_z,
            ceiling: None,
            flush_ride: false,
            airborne: false,
        }
    }

    /// The arm a LIVE tier runs in: a rest op's ceiling, with the two op
    /// priors production's finishing site sets — `flush_ride: true` (flush
    /// ground under a ceiling is the PRIOR pass's machined output, so
    /// riding it is a sub-cusp skim) and
    /// `airborne_links_may_leave_territory: true` (G-LINKVETO: the region
    /// polygon confines CUTTING, not an airborne hop). Both read from
    /// `unified_finish.rs`.
    #[must_use]
    pub const fn rest_op(label: &'static str, safe_z: f64, ceiling: LinkCeiling<'a>) -> Self {
        Self {
            label,
            safe_z,
            ceiling: Some(ceiling),
            flush_ride: true,
            airborne: true,
        }
    }
}

/// [`relink_and_cost_under`] in the fresh-stock regime — the signature five
/// of the six instrument copies shared.
#[must_use]
pub fn relink_and_cost(
    ctx: &CostingContext<'_>,
    raw: Toolpath,
    boundary: &RegionSet<'_>,
    safe_z: f64,
) -> CandidateCost {
    relink_and_cost_under(ctx, raw, boundary, &LinkRegime::fresh_stock(safe_z))
}

/// Apply the production relink to `raw` under `regime`, then price the
/// linked motion with the F-034 integrator.
#[must_use]
pub fn relink_and_cost_under(
    ctx: &CostingContext<'_>,
    raw: Toolpath,
    boundary: &RegionSet<'_>,
    regime: &LinkRegime<'_>,
) -> CandidateCost {
    let link_kinematics = ctx.kinematics.map(|k| LinkKinematics {
        kinematics: *k,
        max_feed_mm_min: ctx.feeds.max_feed_mm_min,
        rapid_feed_mm_min: ctx.feeds.rapid_feed_mm_min,
    });
    let params = crate::surface_link::RelinkParams {
        hookup_distance: 25.0,
        stock_to_leave: 0.0,
        sampling: 0.5,
        feed_rate: ctx.feeds.feed_mm_min,
        plunge_rate: ctx.feeds.plunge_mm_min,
        safe_z: regime.safe_z,
        link_kinematics: link_kinematics.as_ref(),
        reorder: true,
        boundary: Some(boundary),
        link_ceiling: regime.ceiling,
        flush_ride: regime.flush_ride,
        airborne_links_may_leave_territory: regime.airborne,
    };
    let (linked, report) = crate::surface_link::relink_fragments(
        crate::toolpath_spans::AnnotatedToolpath::new(raw),
        ctx.mesh,
        ctx.index,
        ctx.cutter,
        &params,
    );
    let mut channels = crate::transform_provenance::ReconcileSet::new(None, None);
    let toolpath = linked.reconcile(&mut channels).into_inner().toolpath;
    let time_s = match ctx.kinematics {
        Some(kinematics) => compute_cycle_time(
            &toolpath,
            kinematics,
            ctx.feeds.max_feed_mm_min,
            ctx.feeds.rapid_feed_mm_min,
        ),
        None => f64::NAN,
    };
    CandidateCost {
        moves: toolpath.moves.len(),
        cutting_mm: toolpath.total_cutting_distance(),
        rapid_mm: toolpath.total_rapid_distance(),
        time_s,
        fragments: report.fragments,
        linked: report.surface_links,
        kept_retracts: report.retract_links,
        slower_than_retract: report.slower_than_retract,
        ceiling_above_safe_z: report.ceiling_above_safe_z,
        path: toolpath,
    }
}
