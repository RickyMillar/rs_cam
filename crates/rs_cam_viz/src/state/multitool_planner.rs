//! Dialog state for the multi-tool finishing planner (Phase U of
//! `planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md`).
//!
//! # What this type is
//!
//! An editor for exactly one core value —
//! [`rs_cam_core::session::MultitoolPlanSpec`] — plus the lifecycle of the
//! preview that lets the operator veto it before anything is generated
//! (§3.1). [`MultitoolPlannerState::to_spec`] is the whole mapping, and it is
//! the only place dialog fields become plan fields: a second spelling in an
//! Apply handler is how a dial ends up previewing one thing and planning
//! another.
//!
//! # The coarseness slider
//!
//! The operator's ruling (2026-08-27) is ONE knob spanning "lots of small
//! regions" ↔ "a few large ones", with the raw dials behind an advanced
//! flyout. The knob is [`rs_cam_core::tier_islands::TierIslandParams::coarseness`],
//! which scales the DERIVED close radius and min-island area from their
//! `for_tool` baselines — linearly and quadratically respectively, because
//! they are one length scale expressed once as a radius and once as its
//! square. An explicitly typed raw dial is taken verbatim at every
//! coarseness, so setting one turns the slider off *for that dial*; the
//! flyout says so, and this state carries them as `Option` for exactly that
//! reason.
//!
//! # Why the tool list is a snapshot
//!
//! A preview runs on the Optimize worker lane, which takes ownership of the
//! `ProjectSession` for the duration (the same move `open_optimize_project`
//! makes). While it is out, the main thread holds an empty placeholder, so a
//! dialog that read tool names live would blank mid-run. The rows are
//! captured on open instead — the same call `ToolLibraryModalState` makes,
//! for the same reason.
//!
//! # Nothing here auto-locks
//!
//! Every numeric field is a plain editable control. The Feeds tab's rule (no
//! background field locking; explicit buttons instead) applies to this dialog
//! too: the operator types a number and it stays typed until they change it.

use std::time::{Duration, Instant};

use rs_cam_core::session::{MultitoolPlanSpec, MultitoolPreview};
use rs_cam_core::tier_islands::{COARSENESS_MAX, COARSENESS_MIN, TierIslandParams};
use rs_cam_core::tier_map::ResidualTreatment;

/// Smallest planning cell (mm) the dialog will accept.
///
/// **Not a style choice.** A full-grid drop-cutter map costs ≈ 125 s per
/// ladder tool at 0.15 mm on the reference board, and the plan bans that band
/// outright (`ORCHESTRATION_PLAN.md` §1: "plan tiers at 0.3–0.6 mm, NEVER
/// 0.15"). 0.2 is the floor because it is the first value above the banned
/// band, not because 0.2 is cheap.
pub const MIN_PLAN_CELL_MM: f64 = 0.2;

/// Largest planning cell (mm). Past this an island is coarser than the
/// features the operator is trying to separate and the preview stops being a
/// picture of the part.
pub const MAX_PLAN_CELL_MM: f64 = 1.0;

/// How long an island-only dial must sit still before the preview re-runs.
///
/// The re-run is a full [`rs_cam_core::session::ProjectSession::preview_multitool_plan`]
/// call, but with the map key unchanged the walk is a memo hit and only the
/// morphology re-runs — so this debounce is what separates "interactive" from
/// "a queue of stale previews".
pub const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(150);

/// One row of the tool list — a drawer tool the ladder may include.
#[derive(Debug, Clone)]
pub struct PlannerToolRow {
    /// Session tool id (`ToolConfig::id.0`).
    pub tool_id: usize,
    pub name: String,
    /// Tip-sphere radius (mm), from
    /// [`rs_cam_core::tool::MillingCutter::cusp_radius_mm`] — **never**
    /// `radius()`. On a tapered ball `radius()` is the shank, which would
    /// call the project's Ø1-tip finisher the coarsest tool on the ladder.
    pub cusp_radius_mm: f64,
    pub selected: bool,
    /// Which operation cuts this tool's tier (regions are unchanged by
    /// this — territory comes from the tier map).
    pub strategy: rs_cam_core::session::TierStrategy,
}

/// Lifecycle of one preview run, mirroring
/// [`crate::state::OptimizeProjectStatus`]'s shape. The two rode one worker
/// lane until WP14b moved this one to the `Job` lane; the shape stays.
#[derive(Debug, Clone)]
pub enum MultitoolPreviewStatus {
    /// Nothing computed yet.
    Idle,
    /// The walk is on the `Job` lane. Cancel is THIS submit's own flag,
    /// which the walk polls once per grid row — not the lane's, because
    /// that lane is FIFO and shared with the MCP surface.
    Loading,
    /// A tier map and its islands, ready to be vetoed.
    Ready(Box<MultitoolPreview>),
    /// The core refused, rendered.
    Failed(String),
}

/// The dials that decide which tier MAP is built, as an equality key.
///
/// The island dials are deliberately absent: changing one re-runs the
/// morphology against the SAME cached map, which is what makes the coarseness
/// slider interactive. Changing anything in here is a memo miss and costs a
/// full walk, and the dialog says so before the operator drags it.
///
/// This is an echo of `tier_map_cache`'s real key, not the key itself — it
/// cannot see mesh identity. It drives a *hint*, never a correctness
/// decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TierMapKeyEcho {
    tool_ids: Vec<usize>,
    cell_mm: u64,
    tolerance_mm: u64,
    margin_mm: u64,
    treatment: ResidualTreatment,
}

/// Everything the planner dialog edits and remembers.
#[derive(Debug, Clone)]
pub struct MultitoolPlannerState {
    /// `false` = closed but remembered. A veto re-opens onto the same ladder
    /// and dials rather than a fresh dialog (§3.1: "reject re-opens the
    /// ladder dialog").
    pub open: bool,
    /// Drawer snapshot; see the module doc.
    pub tools: Vec<PlannerToolRow>,
    pub setup_index: usize,
    pub model_id: usize,
    pub model_name: String,

    // ── Main dials ──────────────────────────────────────────────────
    /// The single "region coarseness" knob. See the module doc.
    pub coarseness: f64,
    /// Seam-blend band (mm) each fine tier's islands grow by, into the
    /// coarser tier's territory. Rendered in the preview as its own tint.
    pub overlap_mm: f64,
    /// Target cusp height (mm) every tier is dialled to.
    pub cusp_height_mm: f64,
    /// Residual tolerance (mm) the tier labels are decided at.
    pub tolerance_mm: f64,

    // ── Advanced flyout ─────────────────────────────────────────────
    // Whether the flyout is expanded is egui's own collapsing-header memory,
    // keyed on the header's id — not a field here. A second copy of that bit
    // would be one more thing to keep in step for no gain.
    /// Planning grid cell (mm). Clamped to
    /// [`MIN_PLAN_CELL_MM`]..=[`MAX_PLAN_CELL_MM`] by [`Self::to_spec`].
    pub cell_mm: f64,
    pub margin_mm: f64,
    /// `None` = derive from the tier's own cusp radius and scale by
    /// coarseness. `Some` = verbatim, coarseness ignored for this dial.
    pub close_radius_mm: Option<f64>,
    /// See [`Self::close_radius_mm`].
    pub min_region_area_mm2: Option<f64>,
    pub max_regions_per_tier: usize,
    pub rim_erosion_mm: f64,
    pub treatment: ResidualTreatment,
    /// Tier 0 skips the fine tiers' owned islands instead of sweeping the
    /// whole board. Mirrors `MultitoolPlanSpec::coarse_skips_fine_islands`
    /// (the trade-off is documented there).
    pub coarse_skips_fine_islands: bool,
    /// Every emitted tier's shallow band splits into monotone cells on its
    /// own raster lattice, rotated to the region's PCA-minor axis where the
    /// elongation gate passes. Mirrors
    /// `MultitoolPlanSpec::monotone_cell_decomposition` (measured value and
    /// caveats are documented there).
    pub monotone_cell_decomposition: bool,

    // ── Preview lifecycle ───────────────────────────────────────────
    pub status: MultitoolPreviewStatus,
    /// Map key the held preview was built at, for the cached/rebuilding hint.
    pub previewed_key: Option<TierMapKeyEcho>,
    /// Map key of the walk currently in flight.
    ///
    /// Captured at SUBMIT, not at completion: a dial the operator moved while
    /// the walk was running belongs to the next preview, and stamping the
    /// arriving one with it would report a stale map as cached.
    pub requested_key: Option<TierMapKeyEcho>,
    /// Bumped on every Ready. The viewport overlay's upload key, so a
    /// re-preview rebuilds the mesh and a checkbox toggle does not.
    pub preview_generation: u64,
    /// Set when an island-only dial moves; cleared when the debounced
    /// re-preview is requested.
    pub dirty_since: Option<Instant>,
    /// A refusal from Apply, rendered in the dialog rather than as a
    /// notification that scrolls away while the dialog is still up.
    pub apply_error: Option<String>,
}

impl MultitoolPlannerState {
    /// A fresh dialog over `tools`, with the plan's own defaults.
    #[must_use]
    pub fn new(
        tools: Vec<PlannerToolRow>,
        setup_index: usize,
        model_id: usize,
        model_name: String,
    ) -> Self {
        let defaults = MultitoolPlanSpec::default();
        Self {
            open: true,
            tools,
            setup_index,
            model_id,
            model_name,
            coarseness: defaults.islands.coarseness,
            overlap_mm: defaults.islands.overlap_mm,
            cusp_height_mm: defaults.cusp_height_mm,
            tolerance_mm: defaults.tolerance_mm,
            cell_mm: defaults.cell_mm,
            margin_mm: defaults.margin_mm,
            close_radius_mm: defaults.islands.close_radius_mm,
            min_region_area_mm2: defaults.islands.min_region_area_mm2,
            max_regions_per_tier: defaults.islands.max_regions_per_tier,
            rim_erosion_mm: defaults.islands.rim_erosion_mm,
            treatment: defaults.treatment,
            coarse_skips_fine_islands: defaults.coarse_skips_fine_islands,
            monotone_cell_decomposition: defaults.monotone_cell_decomposition,
            status: MultitoolPreviewStatus::Idle,
            previewed_key: None,
            requested_key: None,
            preview_generation: 0,
            dirty_since: None,
            apply_error: None,
        }
    }

    /// Checked tool ids, in the row order the dialog shows.
    ///
    /// The order does NOT matter and must not be relied on: the core ladder
    /// sorts coarse → fine on cusp radius itself. The dialog sorts its rows
    /// the same way purely so the operator reads the ladder in the order it
    /// will run.
    #[must_use]
    pub fn selected_tool_ids(&self) -> Vec<usize> {
        self.tools
            .iter()
            .filter(|t| t.selected)
            .map(|t| t.tool_id)
            .collect()
    }

    /// Why Preview / Apply are refused, in one line for the operator, or
    /// `None` when the ladder is usable.
    ///
    /// Two distinct tools is the core's own precondition; **distinct cusp
    /// radii** is this dialog's: two tools of equal tip radius produce a
    /// ladder whose finer tier can never claim a cell the coarser one
    /// missed, i.e. an empty fine tier and a wasted tool change.
    #[must_use]
    pub fn blocking_reason(&self) -> Option<String> {
        let selected: Vec<&PlannerToolRow> = self.tools.iter().filter(|t| t.selected).collect();
        if selected.len() < 2 {
            return Some(format!(
                "Tick at least two tools — {} ticked. A one-tool ladder is an ordinary \
                 finish pass.",
                selected.len()
            ));
        }
        let mut radii: Vec<f64> = selected.iter().map(|t| t.cusp_radius_mm).collect();
        radii.sort_by(f64::total_cmp);
        if radii.windows(2).any(|pair| {
            pair.first()
                .zip(pair.get(1))
                .is_some_and(|(a, b)| (b - a).abs() < 1e-9)
        }) {
            return Some(
                "Two ticked tools have the same tip radius — the finer tier would claim \
                 nothing the coarser one missed."
                    .to_owned(),
            );
        }
        None
    }

    /// The dials that decide the tier MAP, as an equality key. See
    /// [`TierMapKeyEcho`].
    #[must_use]
    pub fn map_key(&self) -> TierMapKeyEcho {
        let mut tool_ids = self.selected_tool_ids();
        tool_ids.sort_unstable();
        TierMapKeyEcho {
            tool_ids,
            cell_mm: self.effective_cell_mm().to_bits(),
            tolerance_mm: self.tolerance_mm.to_bits(),
            margin_mm: self.margin_mm.to_bits(),
            treatment: self.treatment,
        }
    }

    /// `true` while the held preview's map would be a cache hit for the
    /// dials currently showing — i.e. only island dials have moved.
    #[must_use]
    pub fn map_is_cached(&self) -> bool {
        self.previewed_key
            .as_ref()
            .is_some_and(|key| *key == self.map_key())
    }

    /// The planning cell actually in force, clamped. See
    /// [`MIN_PLAN_CELL_MM`].
    #[must_use]
    pub fn effective_cell_mm(&self) -> f64 {
        if self.cell_mm.is_finite() {
            self.cell_mm.clamp(MIN_PLAN_CELL_MM, MAX_PLAN_CELL_MM)
        } else {
            MultitoolPlanSpec::default().cell_mm
        }
    }

    /// **The dialog → core mapping.** One site; see the module doc.
    #[must_use]
    pub fn to_spec(&self) -> MultitoolPlanSpec {
        MultitoolPlanSpec {
            setup_index: self.setup_index,
            model_id: self.model_id,
            tool_ids: self.selected_tool_ids(),
            cell_mm: self.effective_cell_mm(),
            tolerance_mm: self.tolerance_mm,
            margin_mm: self.margin_mm,
            treatment: self.treatment,
            islands: TierIslandParams {
                close_radius_mm: self.close_radius_mm,
                min_region_area_mm2: self.min_region_area_mm2,
                coarseness: self.coarseness.clamp(COARSENESS_MIN, COARSENESS_MAX),
                overlap_mm: self.overlap_mm.max(0.0),
                max_regions_per_tier: self.max_regions_per_tier,
                rim_erosion_mm: self.rim_erosion_mm.max(0.0),
            },
            cusp_height_mm: self.cusp_height_mm,
            coarse_skips_fine_islands: self.coarse_skips_fine_islands,
            monotone_cell_decomposition: self.monotone_cell_decomposition,
            tier_strategies: self.ladder_tier_strategies(),
        }
    }

    /// The selected rows' strategies in LADDER order (coarse → fine, cusp
    /// radius descending — the same sort the core planner applies), so the
    /// spec's per-tier list lines up with the emitted tiers.
    #[must_use]
    pub fn ladder_tier_strategies(&self) -> Vec<rs_cam_core::session::TierStrategy> {
        let mut rows: Vec<&PlannerToolRow> = self.tools.iter().filter(|t| t.selected).collect();
        rows.sort_by(|a, b| b.cusp_radius_mm.total_cmp(&a.cusp_radius_mm));
        rows.iter().map(|r| r.strategy).collect()
    }

    /// The preview the dialog is holding, if any.
    #[must_use]
    pub fn ready_preview(&self) -> Option<&MultitoolPreview> {
        match &self.status {
            MultitoolPreviewStatus::Ready(preview) => Some(preview.as_ref()),
            _ => None,
        }
    }

    /// `true` while a walk is in flight on the `Job` lane.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        matches!(self.status, MultitoolPreviewStatus::Loading)
    }

    /// Record that an island-only dial moved, starting the debounce window.
    /// A no-op while nothing is previewed — there is no held map to re-cut.
    pub fn mark_island_dial_dirty(&mut self) {
        if self.ready_preview().is_some() {
            self.dirty_since = Some(Instant::now());
        }
    }

    /// `true` once the debounce window has elapsed and a re-preview is due.
    #[must_use]
    pub fn debounce_elapsed(&self) -> bool {
        self.dirty_since
            .is_some_and(|since| since.elapsed() >= PREVIEW_DEBOUNCE)
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

    fn row(tool_id: usize, cusp_radius_mm: f64, selected: bool) -> PlannerToolRow {
        PlannerToolRow {
            tool_id,
            name: format!("tool {tool_id}"),
            cusp_radius_mm,
            selected,
            strategy: rs_cam_core::session::TierStrategy::UnifiedFinish,
        }
    }

    fn state() -> MultitoolPlannerState {
        MultitoolPlannerState::new(
            vec![row(7, 2.0, true), row(9, 1.0, true), row(11, 0.5, false)],
            3,
            5,
            "terrain".to_owned(),
        )
    }

    /// The dial → field mapping, all of it. A dial that lands on the wrong
    /// field previews one plan and emits another, which is the failure this
    /// whole dialog exists to prevent.
    #[test]
    fn every_dial_lands_on_its_own_spec_field() {
        let mut s = state();
        s.coarseness = 3.5;
        s.overlap_mm = 1.25;
        s.cusp_height_mm = 0.045;
        s.tolerance_mm = 0.07;
        s.cell_mm = 0.55;
        s.margin_mm = 0.9;
        s.max_regions_per_tier = 11;
        s.rim_erosion_mm = 3.0;
        s.treatment = ResidualTreatment::Raw;
        s.coarse_skips_fine_islands = true;
        s.monotone_cell_decomposition = true;

        let spec = s.to_spec();
        assert_eq!(spec.setup_index, 3);
        assert_eq!(spec.model_id, 5);
        assert_eq!(
            spec.tool_ids,
            vec![7, 9],
            "unticked rows stay off the ladder"
        );
        assert!((spec.cell_mm - 0.55).abs() < 1e-12);
        assert!((spec.tolerance_mm - 0.07).abs() < 1e-12);
        assert!((spec.margin_mm - 0.9).abs() < 1e-12);
        assert!((spec.cusp_height_mm - 0.045).abs() < 1e-12);
        assert_eq!(spec.treatment, ResidualTreatment::Raw);
        assert!((spec.islands.coarseness - 3.5).abs() < 1e-12);
        assert!((spec.islands.overlap_mm - 1.25).abs() < 1e-12);
        assert_eq!(spec.islands.max_regions_per_tier, 11);
        assert!((spec.islands.rim_erosion_mm - 3.0).abs() < 1e-12);
        assert!(spec.coarse_skips_fine_islands);
        assert!(
            spec.monotone_cell_decomposition,
            "C2's dial must reach the emitted plan, not stop at the dialog"
        );
    }

    /// A raw dial left alone stays `None` — which is what keeps the
    /// coarseness slider in charge of it. Typing one produces `Some`, and the
    /// core then takes it verbatim at every coarseness.
    #[test]
    fn advanced_overrides_are_none_until_typed() {
        let mut s = state();
        let spec = s.to_spec();
        assert_eq!(spec.islands.close_radius_mm, None);
        assert_eq!(spec.islands.min_region_area_mm2, None);

        s.close_radius_mm = Some(0.83);
        s.min_region_area_mm2 = Some(17.5);
        let spec = s.to_spec();
        assert_eq!(spec.islands.close_radius_mm, Some(0.83));
        assert_eq!(spec.islands.min_region_area_mm2, Some(17.5));
    }

    /// The plan bans 0.15 mm maps outright (≈ 125 s per ladder tool). The
    /// dialog clamps rather than trusting a typed number.
    #[test]
    fn the_planning_cell_is_clamped_out_of_the_banned_band() {
        let mut s = state();
        s.cell_mm = 0.15;
        assert!((s.to_spec().cell_mm - MIN_PLAN_CELL_MM).abs() < 1e-12);
        s.cell_mm = 5.0;
        assert!((s.to_spec().cell_mm - MAX_PLAN_CELL_MM).abs() < 1e-12);
        s.cell_mm = f64::NAN;
        assert!(s.to_spec().cell_mm.is_finite());
    }

    #[test]
    fn a_ladder_under_two_tools_is_blocked_with_a_reason() {
        let mut s = state();
        s.tools[1].selected = false;
        let reason = s.blocking_reason().expect("one tool is not a ladder");
        assert!(reason.contains("at least two"), "got: {reason}");
    }

    #[test]
    fn two_tools_of_equal_tip_radius_are_blocked_with_a_reason() {
        let mut s = state();
        s.tools[1].cusp_radius_mm = 2.0;
        let reason = s.blocking_reason().expect("equal radii is not a ladder");
        assert!(reason.contains("same tip radius"), "got: {reason}");
    }

    #[test]
    fn a_two_radius_ladder_is_not_blocked() {
        assert!(state().blocking_reason().is_none());
    }

    /// The cached/rebuilding hint: island dials must not invalidate the map,
    /// map dials must.
    #[test]
    fn only_map_dials_move_the_map_key() {
        let mut s = state();
        let key = s.map_key();
        s.coarseness = 9.0;
        s.overlap_mm = 6.0;
        s.max_regions_per_tier = 3;
        s.close_radius_mm = Some(2.0);
        assert_eq!(key, s.map_key(), "island dials re-cut the SAME map");

        s.tolerance_mm = 0.09;
        assert_ne!(key, s.map_key(), "a tolerance change is a fresh walk");

        let mut s = state();
        s.cell_mm = 0.5;
        assert_ne!(key, s.map_key());

        let mut s = state();
        s.tools[2].selected = true;
        assert_ne!(key, s.map_key(), "the ladder is part of the map key");
    }

    /// The key is order-insensitive on the ladder, because the core sorts it.
    #[test]
    fn the_map_key_does_not_depend_on_row_order() {
        let a = MultitoolPlannerState::new(
            vec![row(7, 2.0, true), row(9, 1.0, true)],
            0,
            0,
            String::new(),
        );
        let b = MultitoolPlannerState::new(
            vec![row(9, 1.0, true), row(7, 2.0, true)],
            0,
            0,
            String::new(),
        );
        assert_eq!(a.map_key(), b.map_key());
    }

    /// Debounce is armed only against a held map — there is nothing to
    /// re-cut before the first preview lands.
    #[test]
    fn the_island_debounce_needs_something_to_re_cut() {
        let mut s = state();
        s.mark_island_dial_dirty();
        assert!(
            s.dirty_since.is_none(),
            "no preview held, nothing to re-cut"
        );
    }
}
