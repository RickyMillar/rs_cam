//! GUI-only runtime overlay state.
//!
//! These types hold presentation and interaction state that is NOT part of
//! the persisted project or the core compute model.  They sit alongside
//! `ProjectSession` in `AppState`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::feeds::{FeedsError, FeedsResult};
use rs_cam_core::geo::P3;
use rs_cam_core::maps::reach_map::{ReachMap, ReachRamp};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};

use crate::render::upload_cache::{self, Refresh, SlotInputs};

use super::toolpath::ToolpathId;

// Re-export ComputeStatus from core (canonical definition).
pub use rs_cam_core::compute::config::ComputeStatus;

// Re-export ToolpathResult from the existing toolpath module.
pub use super::toolpath::ToolpathResult;

// ── Per-toolpath runtime state ────────────────────────────────────────

/// Where the last generation of one toolpath stands, with its result.
///
/// F031 (W9b): the run record held `status` and `result` as two fields
/// that had to agree, and a site cleared the result and left `Done`. Then
/// the status bar counted the op as done, and the prior-stock blocker said
/// "That operation is generated". One enum makes the pair one value:
/// only [`Self::Done`] holds a result.
///
/// Core [`ComputeStatus`] stays the display taxonomy that every surface
/// prints; [`Self::status`] derives it. `ComputeStatus::Disabled` is never
/// derived here, because [`ComputeStatus::effective`] adds it from
/// `enabled`.
#[expect(
    clippy::large_enum_variant,
    reason = "`Done` holds the result inline, as the `Option<ToolpathResult>` field it replaces did. One value per toolpath, so the size is not a cost."
)]
#[derive(Clone)]
pub enum ToolpathRun {
    /// No run, or a cancelled run. No result.
    Pending,
    /// A run is in flight. The previous result is gone.
    Computing,
    /// The last run landed. A late result that the core refused is held
    /// here too, so the viewport still draws it (F2.4).
    Done(ToolpathResult),
    /// A rest operation that waits for the simulated stock of an earlier
    /// operation (A/M11). No result.
    AwaitingPriorStock(rs_cam_core::compute::AwaitingPriorStock),
    /// The run failed. No result.
    Failed(String),
}

impl ToolpathRun {
    /// The display status. Every surface reads this, never a stored copy.
    #[must_use]
    pub fn status(&self) -> ComputeStatus {
        match self {
            Self::Pending => ComputeStatus::Pending,
            Self::Computing => ComputeStatus::Computing,
            Self::Done(_) => ComputeStatus::Done,
            Self::AwaitingPriorStock(block) => ComputeStatus::AwaitingPriorStock(block.clone()),
            Self::Failed(message) => ComputeStatus::Error(message.clone()),
        }
    }

    /// The result of the last run, or `None` when no run landed.
    #[must_use]
    pub fn result(&self) -> Option<&ToolpathResult> {
        match self {
            Self::Done(result) => Some(result),
            Self::Pending | Self::Computing | Self::AwaitingPriorStock(_) | Self::Failed(_) => None,
        }
    }

    /// The result to change in place (the F115 feed re-point).
    pub fn result_mut(&mut self) -> Option<&mut ToolpathResult> {
        match self {
            Self::Done(result) => Some(result),
            Self::Pending | Self::Computing | Self::AwaitingPriorStock(_) | Self::Failed(_) => None,
        }
    }

    /// Whether the last run landed.
    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Done(_))
    }
}

/// GUI-only state for a single toolpath — display, compute status, cached results.
pub struct ToolpathRuntime {
    pub visible: bool,
    pub locked: bool,
    pub auto_regen: bool,
    /// The last run and its result, as one value (F031).
    pub run: ToolpathRun,
    pub stale_since: Option<std::time::Instant>,
    pub feeds_result: Option<FeedsResult>,
    /// The key and the by-products of `feeds_result` (F216). Only
    /// [`FeedsMemo::refresh`] writes the two.
    pub feeds_memo: FeedsMemo,
    pub debug_trace: Option<Arc<rs_cam_core::trace::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::trace::semantic_trace::ToolpathSemanticTrace>>,
    pub debug_trace_path: Option<PathBuf>,
    /// G-B2: the feed caps of the stored result, counted once per result.
    /// Read it through [`Self::caps`].
    pub cap_memo: CapMemo,
    /// G-C: whether `Inside` containment would collapse this toolpath's
    /// rest-regions boundary, asked once per revision and source result.
    /// Read it through [`InsideMemo::answer`].
    pub inside_memo: InsideMemo,
}

impl ToolpathRuntime {
    /// G-B2: the feed caps of the stored result, or `None` when no result
    /// or no capped move. The moves are counted once per result: the
    /// memo keys on the result's toolpath allocation.
    #[must_use]
    pub fn caps(&self) -> Option<CapFigures> {
        self.cap_memo.caps(self.run.result())
    }

    /// Create runtime state for a new toolpath with sensible defaults.
    pub fn new(auto_regen: bool) -> Self {
        Self {
            visible: true,
            locked: false,
            auto_regen,
            run: ToolpathRun::Pending,
            stale_since: None,
            feeds_result: None,
            feeds_memo: FeedsMemo::default(),
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
            cap_memo: CapMemo::default(),
            inside_memo: InsideMemo::default(),
        }
    }
}

// ── Would Inside collapse? (G-C) ──────────────────────────────────────

/// G-C: the answer of core `ProjectSession::boundary_collapse_with` for a
/// rest-regions boundary switched to `Inside`.
#[derive(Debug, Clone, PartialEq)]
pub enum InsideAnswer {
    /// `Inside` keeps area for the tool: the `[Use Inside]` act and the
    /// D-2 pick may write it.
    Keeps,
    /// `Inside` leaves no area (G-BOUNDCOLLAPSE): the refusal sentence.
    Collapses(String),
    /// The inputs do not resolve (for example the source has no result):
    /// the answer is not known, so nothing writes `Inside`.
    Unknown(String),
}

/// The key of an [`InsideAnswer`]: the consumer's revision, the source,
/// and the source's result (a `Weak`, so a regenerated source asks again).
#[derive(Debug)]
struct InsideKey {
    revision: u64,
    source: ToolpathId,
    source_result: Option<std::sync::Weak<rs_cam_core::trace::toolpath_spans::AnnotatedToolpath>>,
}

impl InsideKey {
    fn matches(&self, other: &Self) -> bool {
        self.revision == other.revision
            && self.source == other.source
            && match (&self.source_result, &other.source_result) {
                (Some(a), Some(b)) => std::sync::Weak::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

/// The last [`InsideAnswer`] of one toolpath. The answer costs one input
/// resolution, so the panel asks it once per key, not once per frame. A
/// frame reads through `&GuiState`, so the memo writes through a lock.
#[derive(Debug, Default)]
pub struct InsideMemo(std::sync::Mutex<Option<(InsideKey, InsideAnswer)>>);

impl InsideMemo {
    /// The answer for the consumer at `revision` and the source `source`
    /// with its result `source_result`; `ask` runs only on a new key.
    pub fn answer(
        &self,
        revision: u64,
        source: ToolpathId,
        source_result: Option<&Arc<rs_cam_core::trace::toolpath_spans::AnnotatedToolpath>>,
        ask: impl FnOnce() -> InsideAnswer,
    ) -> InsideAnswer {
        let key = InsideKey {
            revision,
            source,
            source_result: source_result.map(Arc::downgrade),
        };
        let Ok(mut slot) = self.0.lock() else {
            return ask();
        };
        if let Some((stored, answer)) = slot.as_ref()
            && stored.matches(&key)
        {
            return answer.clone();
        }
        let answer = ask();
        *slot = Some((key, answer.clone()));
        answer
    }
}

// ── The feed caps of a stored result (G-B, G-B2) ──────────────────────

/// The feed caps of a stored toolpath (`Move::feed_cap`): the planned
/// moves, not the emitted ones, so a surface that prints them says
/// "planned".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CapFigures {
    pub count: usize,
    pub min_feed_mm_min: f64,
}

impl CapFigures {
    /// `None` when no move of `tp` carries a cap.
    #[must_use]
    pub fn of(tp: &rs_cam_core::toolpath::Toolpath) -> Option<Self> {
        let mut caps = tp.moves.iter().filter_map(|m| m.feed_cap);
        let first = caps.next()?;
        let (count, min_feed_mm_min) = caps.fold((1, first), |(n, low), cap| (n + 1, low.min(cap)));
        Some(Self {
            count,
            min_feed_mm_min,
        })
    }

    /// The one hover line of the Generate row's figures.
    #[must_use]
    pub fn line(self) -> String {
        format!(
            "{} moves at a load cap (planned), lowest {:.0} mm/min",
            self.count, self.min_feed_mm_min
        )
    }
}

/// The [`CapFigures`] of the last result that [`ToolpathRuntime::caps`]
/// read. A frame reads the caps from `&AppState` (the legend, the panel
/// inputs), so the memo writes through a lock. The key is a `Weak` of the
/// result's toolpath: a new result has a new allocation, and a `Weak`
/// keeps the old allocation alive, so its address is not reused.
#[derive(Debug, Default)]
pub struct CapMemo(std::sync::Mutex<Option<CapMemoEntry>>);

#[derive(Debug)]
struct CapMemoEntry {
    key: std::sync::Weak<rs_cam_core::trace::toolpath_spans::AnnotatedToolpath>,
    caps: Option<CapFigures>,
}

impl CapMemo {
    fn caps(&self, result: Option<&super::toolpath::ToolpathResult>) -> Option<CapFigures> {
        let result = result?;
        let Ok(mut slot) = self.0.lock() else {
            return CapFigures::of(result.toolpath());
        };
        if let Some(entry) = slot.as_ref()
            && std::sync::Weak::ptr_eq(&entry.key, &Arc::downgrade(&result.annotated))
        {
            return entry.caps;
        }
        let caps = CapFigures::of(result.toolpath());
        *slot = Some(CapMemoEntry {
            key: Arc::downgrade(&result.annotated),
            caps,
        });
        caps
    }
}

// ── The toolpath panel's feeds memo (F216) ────────────────────────────

/// What the toolpath panel's feeds work reads, read ONCE per frame (F216).
///
/// The Geometry tab (the ⚡ pills) and the Feeds tab (the card) both hand
/// this value to [`FeedsMemo::refresh`]. The key derives from it
/// ([`SlotInputs::key`] destructures it with no `..`), and the build takes
/// only it. So a new input is a compile error in `key` until it is keyed:
/// the W11 shape of `render::upload_cache`.
pub struct FeedsInputs<'a> {
    pub operation: &'a rs_cam_core::compute::catalog::OperationConfig,
    pub tool: &'a rs_cam_core::compute::tool_config::ToolConfig,
    pub material: &'a rs_cam_core::material::Material,
    pub machine: &'a rs_cam_core::machine::MachineProfile,
    pub spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
    /// The box the funnel's runtime-sanity stepover back-off reads (Q1).
    pub model_bbox: Option<&'a rs_cam_core::geo::BoundingBox3>,
}

/// The stored key of [`FeedsMemo`]. Built only by [`SlotInputs::key`].
///
/// `OperationConfig` and `MachineProfile` have no `PartialEq`, so the key
/// holds their `Debug` text (the `Obstacle::effect_digest` idiom). A derived
/// `Debug` prints every field, so the text can only over-key, never miss an
/// edit.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedsKey {
    operation: String,
    tool: rs_cam_core::compute::tool_config::ToolConfig,
    material: rs_cam_core::material::Material,
    machine: String,
    spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
    /// `BoundingBox3` has no `PartialEq`: the two corners.
    model_bbox: Option<[f64; 6]>,
}

impl SlotInputs for FeedsInputs<'_> {
    type Key = FeedsKey;

    fn key(&self) -> FeedsKey {
        let Self {
            operation,
            tool,
            material,
            machine,
            spindle_strategy,
            model_bbox,
        } = self;
        // The vendor LUT is not an input: `embedded_vendor_lut` is one
        // static value for the life of the process.
        FeedsKey {
            operation: format!("{operation:?}"),
            tool: (*tool).clone(),
            material: (*material).clone(),
            machine: format!("{machine:?}"),
            spindle_strategy: *spindle_strategy,
            model_bbox: model_bbox.map(|b| [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z]),
        }
    }
}

/// The funnel dry run the ⚡ pills offer (G-PILLCLAMP). The UI owns it
/// (`ui::properties::pills::pill_previews`); the memo runs it after the
/// calculator. The signature fixes what the dry run may read.
pub type PillDryRun = fn(
    &rs_cam_core::compute::catalog::OperationConfig,
    &FeedsResult,
    &rs_cam_core::compute::tool_config::ToolConfig,
    &rs_cam_core::machine::MachineProfile,
    &rs_cam_core::material::Material,
    Option<&rs_cam_core::geo::BoundingBox3>,
) -> rs_cam_core::feeds::suggest::FieldApplyPreviews;

impl FeedsInputs<'_> {
    /// The calculator and the funnel dry run. This is the work the memo
    /// saves.
    fn build(
        &self,
        dry_run: PillDryRun,
    ) -> Result<(FeedsResult, rs_cam_core::feeds::suggest::FieldApplyPreviews), FeedsError> {
        let Self {
            operation,
            tool,
            material,
            machine,
            spindle_strategy,
            model_bbox,
        } = self;
        let result = rs_cam_core::feeds::suggest::feeds_result_for_operation(
            operation,
            tool,
            material,
            machine,
            rs_cam_core::feeds::embedded_vendor_lut(),
            *spindle_strategy,
        )?;
        let previews = dry_run(operation, &result, tool, machine, material, *model_bbox);
        Ok((result, previews))
    }
}

/// The memo of the toolpath panel's feeds work (F216).
///
/// The panel ran the calculator and the funnel dry run on every frame.
/// The memo runs them once per change of [`FeedsInputs`]. It sits beside
/// `feeds_result` on `ToolpathRuntime` and `ToolpathEntry`; the panel copies
/// both in and back each frame.
#[derive(Debug, Clone, Default)]
pub struct FeedsMemo {
    key: Option<FeedsKey>,
    /// The funnel dry run for the stored result. `None` on a refusal.
    previews: Option<rs_cam_core::feeds::suggest::FieldApplyPreviews>,
    /// The calculator's refusal. The Feeds tab prints it.
    refusal: Option<FeedsError>,
    /// The count of builds: the instrument of the F216 sentry.
    builds: u64,
}

impl FeedsMemo {
    /// The one writer of `result` and of this memo.
    ///
    /// When the key of `inputs` matches the stored key, nothing runs.
    /// Otherwise the calculator and `dry_run` run once, and `result`
    /// holds the answer (`None` on a refusal).
    pub fn refresh(
        &mut self,
        result: &mut Option<FeedsResult>,
        inputs: FeedsInputs<'_>,
        dry_run: PillDryRun,
    ) {
        let Refresh::Build(Some(inputs)) = upload_cache::refresh(&mut self.key, Some(inputs))
        else {
            return;
        };
        self.builds += 1;
        match inputs.build(dry_run) {
            Ok((answer, previews)) => {
                *result = Some(answer);
                self.previews = Some(previews);
                self.refusal = None;
            }
            Err(error) => {
                *result = None;
                self.previews = None;
                self.refusal = Some(error);
            }
        }
    }

    /// The funnel dry run for the stored result.
    pub fn previews(&self) -> Option<&rs_cam_core::feeds::suggest::FieldApplyPreviews> {
        self.previews.as_ref()
    }

    /// Why the calculator refused, when it did.
    pub fn refusal(&self) -> Option<&FeedsError> {
        self.refusal.as_ref()
    }

    /// How many times the calculator ran for this toolpath.
    pub fn builds(&self) -> u64 {
        self.builds
    }
}

// ── Per-setup state (W9 / P-2) ────────────────────────────────────────
//
// `SetupRuntime { datum, model_ids }` used to live here as GUI-only
// overlay state, alongside a byte-identical private copy of the datum
// enums. Both fields are operator intent — how the machine is zeroed and
// which models the setup is allowed to use — and neither had a home on
// the wire, so every save dropped them. They now live on core's
// `SetupData` and are persisted; the Setup properties panel already had
// `&mut SetupData` in hand, so it writes straight through to the session
// and there is no overlay left to keep in sync.

// ── Per-tool reach map overlay (P5) ───────────────────────────────────

/// Where the selected toolpath's reach map is in its life cycle.
///
/// `Idle` means **no map is wanted** — the selection is not a reach-capable
/// operation, or it carries no mesh or no tool. It never means "measured
/// zero"; a measured map that found no surface says so through
/// [`ReachMap::is_measured`], and the panel prints `reach: not measured`
/// rather than a clean percentage.
pub enum ReachStatus {
    Idle,
    Computing,
    Ready(Arc<ReachMap>),
    Failed(String),
}

/// The reach map the viewport is drawing, plus what it was resolved for.
///
/// The map is built on a worker (`ComputeLane::Reach`) because a cold walk is
/// one full-grid drop-cutter pass — seconds, not milliseconds. Nothing here
/// borrows the session: `rs_cam_core::maps::reach_map::ReachMapRequest` carries its
/// own mesh, index and cutter, which is what lets the walk leave the UI
/// thread without lending the session out.
pub struct ReachOverlayState {
    /// The toolpath this answer belongs to. A result for any other toolpath
    /// is a stale supersede and is dropped, never shown as a failure.
    pub toolpath: Option<ToolpathId>,
    pub status: ReachStatus,
    /// Bumped on every map that is not the map already held, and read by
    /// `ReachOverlayUploadKey` — so a result identical to the last one
    /// rebuilds no GPU buffer.
    ///
    /// Compared against [`Self::last_map`], NOT against [`Self::status`]: the
    /// scheduler sets `status = Computing` on submit, so by the time a result
    /// lands `status` never holds a map to compare with, and every arrival
    /// would look new.
    pub generation: u64,
    /// The last map this overlay accepted, kept only as the identity
    /// [`Self::generation`] is compared against.
    ///
    /// It deliberately survives [`Self::clear`]: the reach memo hands back the
    /// same `Arc` for a warm key, so a deselect-and-reselect must not bump the
    /// generation and rebuild a 661 k-triangle buffer. The upload key's
    /// `None` → `Some` transition already forces the rebuild that a genuine
    /// deselect needs, so the generation only has to separate consecutive
    /// live keys.
    pub last_map: Option<Arc<ReachMap>>,
    /// The memo key of the request this answer was asked under, or `None`
    /// when the selected toolpath resolves no request (an operation that a
    /// reach map does not speak about, no mesh, or no tool). Part of the
    /// scheduling key, beside [`Self::toolpath`].
    ///
    /// The key is built from the inputs the map reads: the mesh identity
    /// (the setup transform gives a new mesh `Arc`), the tool shape, the
    /// tolerance from the operation config, the cell and the ids. It is
    /// `rs_cam_core::maps::reach_map_cache::ReachRequestKey`, the core memo's
    /// own key, so the overlay resubmits exactly when the memo would answer
    /// a new question.
    ///
    /// It is NOT `GuiState::edit_counter`. That counter misses each MCP core
    /// command and each controller command that does not call
    /// `mark_edited`, so an MCP `set_tool_param` left the previous tool's
    /// map on screen. It also moved on each unrelated edit, and the overlay
    /// then blinked to `reach: computing…` for no new question.
    pub key: Option<rs_cam_core::maps::reach_map_cache::ReachRequestKey>,
    /// When the in-flight request was submitted, or `None` when none is.
    ///
    /// Read only by the scheduler's stuck-`Computing` recovery, which is why
    /// it is a timestamp rather than a flag: a backend that runs no Reach lane
    /// (every scripted test double) reports an idle lane the instant after a
    /// submit, so an unbounded recovery would resubmit on every pump for ever.
    pub requested_at: Option<std::time::Instant>,
    /// The refined surface and its colours, built on the worker beside
    /// the map ([`reach_overlay_surface`]).
    ///
    /// **On the worker, not in the upload pass**: `vertex_gaps` runs one
    /// spatial-index query per vertex, which on a board-sized terrain is a
    /// visible hitch if it lands on the frame loop. The upload pass only
    /// replays the refinement on the drawn mesh.
    pub surface: Option<Arc<ReachSurface>>,
}

impl ReachOverlayState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            toolpath: None,
            status: ReachStatus::Idle,
            generation: 0,
            last_map: None,
            key: None,
            requested_at: None,
            surface: None,
        }
    }

    /// The map, when one is held for `toolpath`.
    #[must_use]
    pub fn ready_map(&self) -> Option<&Arc<ReachMap>> {
        match &self.status {
            ReachStatus::Ready(map) => Some(map),
            _ => None,
        }
    }

    /// Forget the answer and the key it was asked under. The next scheduler
    /// sweep decides what to ask for.
    ///
    /// [`Self::generation`] and [`Self::last_map`] deliberately survive — see
    /// their docs for why a deselect must not make the next identical map
    /// look new.
    pub fn clear(&mut self) {
        self.toolpath = None;
        self.status = ReachStatus::Idle;
        self.key = None;
        self.requested_at = None;
        self.surface = None;
    }
}

impl Default for ReachOverlayState {
    fn default() -> Self {
        Self::new()
    }
}

/// The reach overlay surface: the measured mesh refined to the map cell,
/// with one colour per refined vertex (round 7, FOLLOW_UPS §12).
///
/// A colour per vertex of the MODEL mesh cannot carry the cell field on a
/// coarse tessellation. A STEP block has 48 vertices, and each one lies on
/// the part outline or at the foot of a wall (not topmost); before R7d the
/// map also abstained on a band inside the outline. So every vertex took the not-measured colour, while
/// the map held 4067 measured cells. The refinement adds vertices at the
/// map cell, so the field shows on the faces.
///
/// The refinement is a list of midpoints, not coordinates. The upload pass
/// replays it on the DRAWN mesh ([`Self::refined_mesh`]): the display
/// transform is affine, and an affine map keeps midpoints. So the measured
/// frame and the display frame need not be the same.
///
/// Round 7c: a triangle that is not up-facing (a wall, an underside) is
/// not measured. It must not share a vertex with a measured triangle, or
/// the GPU interpolates the measured colour across the wall. So each
/// vertex that both kinds use gets a copy for the not-up-facing triangles
/// ([`Self::copies`]), coloured `colors::MODEL_SURFACE`. The upload replays
/// the copies after the midpoints.
#[derive(Debug, Clone)]
pub struct ReachSurface {
    /// The vertex count of the mesh the map was measured on.
    pub base_vertices: usize,
    /// Each added vertex, in order: the midpoint of two earlier vertices.
    /// Added vertex `k` has the index `base_vertices + k`.
    pub midpoints: Vec<[u32; 2]>,
    /// Each copied vertex, in order: a copy of an earlier vertex (round 7c).
    /// Copy `k` has the index `base_vertices + midpoints.len() + k`. Only
    /// the not-up-facing triangles use a copy; only the up-facing ones use
    /// its source. The count is at most the vertex count before the copies,
    /// so [`reach_refine_budget`] does not count it.
    pub copies: Vec<u32>,
    /// The refined triangles, with the winding of the triangles they split.
    pub triangles: Vec<[u32; 3]>,
    /// One colour per refined vertex ([`reach_overlay_colors`]).
    pub colors: Vec<[f32; 3]>,
    /// The ramp of these colours. The legend draws from the same value.
    pub ramp: ReachRamp,
    /// The tip radius (mm) of the cutter, `MillingCutter::cusp_radius_mm`.
    pub tip_radius_mm: f64,
    /// True when the colour scale is tool-relative: full colour at one tip
    /// radius. False when the tip radius is not above the bar, and the
    /// scale keeps the deepest gap of the map ([`reach_display_ramp`]).
    pub tip_relative: bool,
    /// True when the vertex budget stopped the refinement
    /// ([`reach_refine_budget`]). The colours are then coarser than the
    /// cell in some places.
    pub budget_hit: bool,
}

impl ReachSurface {
    /// The refined mesh on the vertices of `mesh`. `mesh` is the measured
    /// mesh, or its image under one affine map (the display transform). It
    /// keeps the vertex order. `None` when the vertex count is not
    /// [`Self::base_vertices`].
    #[must_use]
    pub fn refined_mesh(&self, mesh: &TriangleMesh) -> Option<TriangleMesh> {
        if mesh.vertices.len() != self.base_vertices {
            return None;
        }
        let mut vertices = mesh.vertices.clone();
        vertices.reserve(self.midpoints.len() + self.copies.len());
        for &[a, b] in &self.midpoints {
            let a = *vertices.get(a as usize)?;
            let b = *vertices.get(b as usize)?;
            vertices.push(midpoint(a, b));
        }
        for &source in &self.copies {
            let p = *vertices.get(source as usize)?;
            vertices.push(p);
        }
        Some(TriangleMesh::from_raw(vertices, self.triangles.clone()))
    }
}

/// True when the triangle `a, b, c` faces up (normal Z > 0): the core's
/// population rule. The refinement and the wall split read this one test.
#[must_use]
pub fn reach_up_facing(a: P3, b: P3, c: P3) -> bool {
    (b - a).cross(&(c - a)).z > 0.0
}

fn midpoint(a: P3, b: P3) -> P3 {
    P3::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5)
}

/// The ramp of the overlay colours: the map's own bar and floor, and the
/// full colour at ONE TIP RADIUS of the cutter (round 7).
///
/// The core ramp ends at the deepest gap of the map. On the stepped block
/// that is 8.04 mm against a Ø10 ball, so a 0.5 mm corner gap sat in the
/// first sliver of the ramp. A gap in units of the tip radius reads the
/// same for each tool: a gap of one radius is a whole tip that does not
/// fit, and a gap past it is clamped to the full colour
/// (`ReachRamp::depth_t`). When the tip radius is not above the bar (the
/// bar or the floor), the log ramp has no span, so the scale keeps the
/// map's deepest gap.
#[must_use]
pub fn reach_display_ramp(map: &ReachMap, tip_radius_mm: f64) -> ReachRamp {
    let ramp = map.ramp();
    if tip_scale_applies(map, tip_radius_mm) {
        ReachRamp {
            max_gap_mm: tip_radius_mm,
            ..ramp
        }
    } else {
        ramp
    }
}

/// True when the tip radius is above the bar of `map`, so a log ramp from
/// the bar to one radius has a span.
fn tip_scale_applies(map: &ReachMap, tip_radius_mm: f64) -> bool {
    tip_radius_mm.is_finite() && tip_radius_mm > map.ramp().bar_mm()
}

/// The most vertices the refinement may ADD for `map`: eight per grid
/// cell. It is a guard against a run-away (a mesh with many up-facing
/// layers), not a target.
///
/// The cell field is constant over a cell. A vertex pitch of half a cell
/// samples a feature one cell wide, which is four vertices per cell. The
/// conforming bisection does not place vertices on a lattice, so the guard
/// allows twice that. The stepped block of the round 7 sentry
/// (`a_step_block_shows_the_reach_field_on_its_faces_r7`) prints its count
/// against this budget and asserts that it stays under it.
pub fn reach_refine_budget(map: &ReachMap) -> usize {
    map.grid.nx.saturating_mul(map.grid.ny).saturating_mul(8)
}

/// The XY length of the edge from `a` to `b`. The cell field varies in XY
/// only, so a steep edge that is short in XY needs no split.
fn xy_len(a: P3, b: P3) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn edge_key(a: u32, b: u32) -> [u32; 2] {
    [a.min(b), a.max(b)]
}

/// The bits of one vertex position: a key that is equal only for equal
/// coordinates.
type PointBits = [u64; 3];

fn point_bits(p: P3) -> PointBits {
    [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
}

/// The key of the edge from `a` to `b` by POSITION, not by index. A STEP
/// mesh holds one vertex set per face, so two faces on one edge share
/// positions and not indices.
fn position_key(a: P3, b: P3) -> [PointBits; 2] {
    let (a, b) = (point_bits(a), point_bits(b));
    if a <= b { [a, b] } else { [b, a] }
}

/// The refinement of `mesh` to the cell `cell_mm`: `(vertices, midpoints,
/// triangles, budget_hit)`.
///
/// Conforming longest-edge bisection, in rounds:
///
/// 1. Mark each edge of an up-facing triangle whose XY length is more than
///    one cell. Up-facing is the core's population rule (normal Z > 0); a
///    wall or an underside is not measured, so it needs no vertices of its
///    own. The mark is by position, so it holds for each face on the edge.
/// 2. Split each triangle that holds a marked edge on its longest marked
///    edge, until no triangle holds one. Each index pair takes one
///    midpoint, and each face on a marked edge splits at the same position
///    (a wall too), so the result has no T-junction.
/// 3. Repeat until no edge is marked, or until the added vertices pass
///    `max_added` (checked before each round).
#[expect(
    clippy::type_complexity,
    reason = "a private return tuple, destructured at its one call site"
)]
fn refine_to_cell(
    mesh: &TriangleMesh,
    cell_mm: f64,
    max_added: usize,
) -> (Vec<P3>, Vec<[u32; 2]>, Vec<[u32; 3]>, bool) {
    let mut vertices = mesh.vertices.clone();
    let mut midpoints: Vec<[u32; 2]> = Vec::new();
    let mut triangles = mesh.triangles.clone();
    if !(cell_mm.is_finite() && cell_mm > 0.0) {
        return (vertices, midpoints, triangles, false);
    }
    let corner = |vertices: &[P3], i: u32| vertices.get(i as usize).copied();
    loop {
        let mut marked: std::collections::HashSet<[PointBits; 2]> =
            std::collections::HashSet::new();
        for tri in &triangles {
            let (Some(a), Some(b), Some(c)) = (
                corner(&vertices, tri[0]),
                corner(&vertices, tri[1]),
                corner(&vertices, tri[2]),
            ) else {
                continue;
            };
            if !reach_up_facing(a, b, c) {
                continue;
            }
            for (p, q) in [(a, b), (b, c), (c, a)] {
                if xy_len(p, q) > cell_mm {
                    marked.insert(position_key(p, q));
                }
            }
        }
        if marked.is_empty() {
            return (vertices, midpoints, triangles, false);
        }
        // The budget is checked before each round on the vertices added
        // so far, so the last round may pass it once. The guard is for a
        // run-away, not a hard count. A marked edge adds at most one
        // vertex per face on it: one on a manifold surface, and one per
        // side on a crease of a mesh with one vertex set per face.
        let next_vertices = vertices
            .len()
            .saturating_add(marked.len().saturating_mul(2));
        if midpoints.len() > max_added || next_vertices >= u32::MAX as usize {
            return (vertices, midpoints, triangles, true);
        }
        let mut mid_of: HashMap<[u32; 2], u32> = HashMap::new();
        let mut work = std::mem::take(&mut triangles);
        while let Some(tri) = work.pop() {
            // The longest marked edge, as (length, start, end, opposite).
            let mut best: Option<(f64, u32, u32, u32)> = None;
            for (i, j, k) in [
                (tri[0], tri[1], tri[2]),
                (tri[1], tri[2], tri[0]),
                (tri[2], tri[0], tri[1]),
            ] {
                let (Some(p), Some(q)) = (corner(&vertices, i), corner(&vertices, j)) else {
                    continue;
                };
                if !marked.contains(&position_key(p, q)) {
                    continue;
                }
                let len = xy_len(p, q);
                if best.is_none_or(|(held, ..)| len > held) {
                    best = Some((len, i, j, k));
                }
            }
            let Some((_, i, j, k)) = best else {
                triangles.push(tri);
                continue;
            };
            let key = edge_key(i, j);
            let mid = match mid_of.get(&key) {
                Some(&mid) => mid,
                None => {
                    let (Some(p), Some(q)) = (corner(&vertices, key[0]), corner(&vertices, key[1]))
                    else {
                        triangles.push(tri);
                        continue;
                    };
                    let mid = vertices.len() as u32;
                    vertices.push(midpoint(p, q));
                    midpoints.push(key);
                    mid_of.insert(key, mid);
                    mid
                }
            };
            // (i, j, k) is a rotation of the triangle, so both halves keep
            // its winding. The two half edges are new positions, so they
            // carry no mark in this round.
            work.push([i, mid, k]);
            work.push([mid, j, k]);
        }
    }
}

/// One colour per vertex of `mesh` for the reach overlay, on the ramp
/// `ramp`.
///
/// The single construction site for the overlay's colours, so the worker,
/// the screenshot and the sentry test cannot drift apart. A `NaN` gap (an
/// underside, the foot of a wall, anything off the measured
/// population) takes `colors::MODEL_SURFACE`, the material of the model
/// shaders. The overlay shader lights it as the model shader does, so a
/// not-measured vertex reads exactly as the plain model.
#[must_use]
pub fn reach_overlay_colors(
    map: &ReachMap,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    ramp: ReachRamp,
) -> Vec<[f32; 3]> {
    let (gaps, floors) = map.vertex_gaps_and_floors(mesh, index);
    let mut colors = rs_cam_core::maps::reach_map::reach_colors(&gaps, &floors, ramp);
    for (color, gap) in colors.iter_mut().zip(&gaps) {
        if !gap.is_finite() {
            *color = crate::render::colors::MODEL_SURFACE.array();
        }
    }
    colors
}

/// The reach overlay surface of `map` over `mesh` (the mesh the map was
/// measured on, with its spatial index `index`), for a cutter of tip
/// radius `tip_radius_mm`. See [`ReachSurface`].
#[must_use]
pub fn reach_overlay_surface(
    map: &ReachMap,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    tip_radius_mm: f64,
) -> ReachSurface {
    let ramp = reach_display_ramp(map, tip_radius_mm);
    let (vertices, midpoints, triangles, budget_hit) =
        refine_to_cell(mesh, map.grid.cell_mm, reach_refine_budget(map));
    if budget_hit {
        tracing::warn!(
            added_vertices = midpoints.len(),
            budget = reach_refine_budget(map),
            "reach overlay refinement stopped at its vertex budget; the colours are coarser than the cell in places"
        );
    }
    let mut colors = if midpoints.is_empty() {
        reach_overlay_colors(map, mesh, index, ramp)
    } else {
        let refined = TriangleMesh::from_raw(vertices.clone(), triangles.clone());
        let refined_index = SpatialIndex::build_auto(&refined);
        reach_overlay_colors(map, &refined, &refined_index, ramp)
    };
    let (copies, triangles) = split_unmeasured(&vertices, triangles, &mut colors);
    ReachSurface {
        base_vertices: mesh.vertices.len(),
        midpoints,
        copies,
        triangles,
        colors,
        ramp,
        tip_radius_mm,
        tip_relative: tip_scale_applies(map, tip_radius_mm),
        budget_hit,
    }
}

/// Give the not-up-facing triangles their own vertices, coloured
/// `colors::MODEL_SURFACE` (round 7c): `(copies, triangles)`.
///
/// A wall triangle that shares a vertex with a measured (up-facing)
/// triangle takes that vertex's measured colour, and the GPU interpolates
/// it across the whole wall. Each vertex that both kinds use gets ONE copy,
/// which the not-up-facing triangles share; a vertex that only
/// not-up-facing triangles use keeps its index. Both take the model
/// material. `colors` gains one entry per copy. A triangle with a corner
/// out of range keeps its indices, as the refinement leaves it.
fn split_unmeasured(
    vertices: &[P3],
    mut triangles: Vec<[u32; 3]>,
    colors: &mut Vec<[f32; 3]>,
) -> (Vec<u32>, Vec<[u32; 3]>) {
    let grey = crate::render::colors::MODEL_SURFACE.array();
    let corners = |tri: &[u32; 3]| -> Option<[P3; 3]> {
        Some([
            *vertices.get(tri[0] as usize)?,
            *vertices.get(tri[1] as usize)?,
            *vertices.get(tri[2] as usize)?,
        ])
    };
    let measured_tri =
        |tri: &[u32; 3]| corners(tri).is_some_and(|[a, b, c]| reach_up_facing(a, b, c));
    let mut used_by_measured = vec![false; vertices.len()];
    for tri in triangles.iter().filter(|tri| measured_tri(tri)) {
        for &i in tri {
            if let Some(slot) = used_by_measured.get_mut(i as usize) {
                *slot = true;
            }
        }
    }
    let mut copies: Vec<u32> = Vec::new();
    let mut copy_of: HashMap<u32, u32> = HashMap::new();
    for tri in &mut triangles {
        if corners(tri).is_none() || measured_tri(tri) {
            continue;
        }
        for index in tri.iter_mut() {
            let source = *index;
            if used_by_measured
                .get(source as usize)
                .copied()
                .unwrap_or(false)
            {
                *index = *copy_of.entry(source).or_insert_with(|| {
                    let copy = (vertices.len() + copies.len()) as u32;
                    copies.push(source);
                    copy
                });
            } else if let Some(color) = colors.get_mut(source as usize) {
                *color = grey;
            }
        }
    }
    colors.extend(std::iter::repeat_n(grey, copies.len()));
    (copies, triangles)
}

/// The scale and floor note of the reach overlay, in tool units: where the
/// colour is full, and how large the grid floor is against the tip radius.
/// The Geometry tab and the legend chip print this one string.
#[must_use]
pub fn reach_scale_note(map: &ReachMap, surface: &ReachSurface) -> String {
    let radius = surface.tip_radius_mm;
    let floor = map.discretisation_floor_mm;
    let scale = if surface.tip_relative {
        format!("full colour at one tip radius R = {radius:.2} mm")
    } else {
        format!(
            "full colour at the deepest gap {:.2} mm (the tip radius {radius:.2} mm is not above the bar)",
            surface.ramp.max_gap_mm
        )
    };
    let floor_in_r = if radius > 0.0 {
        format!(" = {:.2} R", floor / radius)
    } else {
        String::new()
    };
    // R7d: the map measures up to the part outline, so grey is only the
    // surface a top-down measure cannot see.
    format!(
        "{scale} \u{00B7} grid floor {floor:.3} mm{floor_in_r}: a reported gap under the floor is discretisation, not geometry \u{00B7} grey: not measured (walls, undersides)"
    )
}

// ── Project-level GUI state ───────────────────────────────────────────

/// User-controlled overrides for the tool-load export gate. Each flag bypasses
/// a *distinct* class of refusal — they are deliberately not collapsed into a
/// single "I accept the risk" toggle. Default is the strict policy: refuse on
/// either Exceeds or Unmodeled.
#[derive(Default, Debug, Clone, Copy)]
pub struct ToolLoadOverrides {
    /// Bypass `Unmodeled` refusals (criterion couldn't be evaluated honestly).
    pub accept_unmodeled: bool,
    /// Bypass `Exceeds` refusals (criterion was modeled and predicted to break
    /// the tool, exceed power, or have unsafe stickout).
    pub accept_exceeded: bool,
}

impl ToolLoadOverrides {
    /// Translate to the core policy struct consumed by `export_gcode_checked`.
    pub fn as_policy(&self) -> rs_cam_core::gcode::ToolLoadExportPolicy {
        rs_cam_core::gcode::ToolLoadExportPolicy {
            accept_unmodeled: self.accept_unmodeled,
            accept_exceeded: self.accept_exceeded,
        }
    }
}

/// Whether THIS export may emit an operation's PREVIOUS geometry — the
/// result of the generation before the operator edited the operation.
///
/// Deliberately not a `bool`. An operator who overrides this gate is
/// choosing to cut geometry that does not match the parameters on
/// screen, and a bare `true` at a call site says none of that.
///
/// Off by default, and held here beside [`ToolLoadOverrides`] rather than
/// in the project file: it is a decision about one export, not a property
/// of the job, so it resets whenever a project is loaded (both loaders
/// build a fresh `GuiState`). R0.1 §7 Q5 asked the operator to choose
/// between this and a persisted project setting and has not been
/// answered; this follows R0.1's own recommendation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StaleResultPolicy {
    /// The default. An operation edited since its last generation blocks
    /// the export, and the refusal names it.
    #[default]
    Refuse,
    /// Emit the previous generation's geometry for an edited operation.
    /// The file will not match the parameters on screen.
    AcceptPreviousGeometry,
}

impl StaleResultPolicy {
    /// Whether an edited operation may be exported under this policy.
    pub fn accepts_previous_geometry(self) -> bool {
        matches!(self, Self::AcceptPreviousGeometry)
    }
}

/// GUI-only project-level state.
pub struct GuiState {
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub edit_counter: u64,
    /// Per-toolpath GUI runtime state, keyed by toolpath semantic ID.
    pub toolpath_rt: HashMap<rs_cam_core::ToolpathId, ToolpathRuntime>,
    /// User-toggled overrides for the tool-load export gate. Reset on project load.
    pub tool_load_overrides: ToolLoadOverrides,
    /// G-STALEXPORT: whether the next export may emit an edited
    /// operation's previous geometry. Reset on project load, like
    /// [`Self::tool_load_overrides`]; never written to the project file.
    pub stale_export: StaleResultPolicy,
    /// Recently changed parameters from MCP, with timestamp for fade-out.
    /// Key: "toolpath_{id}_{param}" or "tool_{id}_{param}" or "stock_{param}"
    #[cfg(feature = "mcp")]
    pub mcp_highlights: HashMap<String, std::time::Instant>,
    /// One-shot toolpath-properties tab override, set by the MCP
    /// `set_ui_view` tool: `(target toolpath, tab)`. Consumed the
    /// next time the properties panel renders *that* toolpath — scoping
    /// to the target makes the override survive whatever frame the
    /// workspace-switch / selection events land on (pre-fix an
    /// intervening render of the previously selected toolpath consumed
    /// it and persisted the tab onto the wrong toolpath).
    ///
    /// UI-06: the tab is the [`crate::ui::properties::ToolpathTab`] enum,
    /// not its agent-facing key. The MCP boundary parses the key once;
    /// a GUI-internal writer names the variant it means.
    /// Not cfg-gated on `mcp` so the properties panel can consume it
    /// unconditionally.
    pub pending_toolpath_tab: Option<(
        crate::state::toolpath::ToolpathId,
        crate::ui::properties::ToolpathTab,
    )>,
    /// The properties tab on screen and its toolpath (live look round 3):
    /// the toolpath panel writes it each frame it draws. The height planes
    /// draw while it is the Heights tab of the selected toolpath
    /// (`state::viewport::height_planes_drawn`).
    pub active_toolpath_tab: Option<(
        crate::state::toolpath::ToolpathId,
        crate::ui::properties::ToolpathTab,
    )>,
    /// Per-tool reach map for the selected toolpath (P5). Scheduled by
    /// `AppController::process_reach_overlay`, filled by the Reach compute
    /// lane, drawn by `ViewportCallback::show_reach_overlay`.
    pub reach_overlay: ReachOverlayState,
    /// Resumable export-wizard settings.
    ///
    /// WP6b, plan section 19 ruling 5. The record lived on
    /// `ProjectSession` behind a `wizard_mut` hatch. Nothing saved it and
    /// the project loader reset it on every load, so it is GUI state and
    /// not project data. It sits here rather than on `AppState` because
    /// `GuiState` is the argument every export door already takes, so
    /// `io::export::overlay_for` reads it without a new parameter.
    pub wizard: crate::state::wizard::WizardState,
    /// H2: the memo of the core shoe plan. The toolpath panel and the run
    /// sheet read the plan on every frame; the memo builds it once per
    /// change of an input.
    pub shoe_plan: crate::state::shoe_plan::ShoePlanMemo,
    /// P4.3: the pick mode and the `+ Rule` draft of the selection
    /// section. View state only: a project load resets it with the rest of
    /// this struct, and no file holds it.
    pub selection_ui: crate::state::pick::SelectionUi,
}

impl GuiState {
    pub fn new() -> Self {
        Self {
            file_path: None,
            dirty: false,
            edit_counter: 0,
            toolpath_rt: HashMap::new(),
            tool_load_overrides: ToolLoadOverrides::default(),
            stale_export: StaleResultPolicy::default(),
            #[cfg(feature = "mcp")]
            mcp_highlights: HashMap::new(),
            pending_toolpath_tab: None,
            active_toolpath_tab: None,
            reach_overlay: ReachOverlayState::new(),
            wizard: crate::state::wizard::WizardState::default(),
            shoe_plan: crate::state::shoe_plan::ShoePlanMemo::default(),
            selection_ui: crate::state::pick::SelectionUi::default(),
        }
    }

    /// Mark the project as having unsaved changes.
    pub fn mark_edited(&mut self) {
        self.dirty = true;
        self.edit_counter += 1;
    }

    /// Get or create a toolpath runtime entry.
    pub fn toolpath_rt_or_default(&mut self, id: rs_cam_core::ToolpathId) -> &mut ToolpathRuntime {
        self.toolpath_rt
            .entry(id)
            .or_insert_with(|| ToolpathRuntime::new(true))
    }
}

impl Default for GuiState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use rs_cam_core::gcode::{PostConfig, PostFormat};

    /// W9 / P-1. The Post panel's dropdown used to reset from grblHAL to
    /// GRBL on every reload, because one of two hand-written translations
    /// had no `"grblhal"` arm.
    ///
    /// UI-07 deleted both translations: the session holds
    /// `gcode::PostConfig` itself. The round trip that remains is the ONE
    /// that can still lose a dialect — serialise the config and read it
    /// back, which is what saving and loading a project does.
    #[test]
    fn every_post_format_survives_the_session_round_trip() {
        for &format in PostFormat::ALL {
            let post = PostConfig {
                format,
                ..PostConfig::default()
            };
            let json = serde_json::to_string(&post).expect("the post config serialises");
            let back: PostConfig = serde_json::from_str(&json).expect("and reads back");
            assert_eq!(
                back.format, format,
                "{format:?} did not survive the project-file round trip (wrote {json})"
            );
        }
    }

    /// The serialized spelling is part of the file format — pin it so the
    /// P-1 fix cannot be "rename the token".
    ///
    /// UI-07 made serde WRITE these same tokens, so the two halves the
    /// pin covers are now one. A project written before UI-07 reads back
    /// unchanged.
    #[test]
    fn the_written_post_tokens_are_the_shipped_spellings() {
        let tokens: Vec<&str> = PostFormat::ALL.iter().map(|f| f.to_token()).collect();
        assert_eq!(tokens, vec!["grbl", "grblhal", "linuxcnc", "mach3"]);

        for (&format, token) in PostFormat::ALL.iter().zip(&tokens) {
            let written = serde_json::to_string(&format).expect("the format serialises");
            assert_eq!(
                written,
                format!("\"{token}\""),
                "serde must write the token the project file already holds"
            );
        }
    }

    /// W4 (F090, no legacy): the underscore spellings are gone; each one
    /// fails to read back instead of loading as its post.
    #[test]
    fn the_underscore_post_spellings_are_refused() {
        for spelling in ["\"grbl_hal\"", "\"linux_cnc\""] {
            assert!(
                serde_json::from_str::<PostFormat>(spelling).is_err(),
                "{spelling} must not load"
            );
        }
    }

    /// A project file with no post block at all loads as the default
    /// rather than failing.
    #[test]
    fn an_absent_post_block_reads_back_as_the_default() {
        let back: PostConfig = serde_json::from_str("{}").expect("an empty post block loads");
        assert_eq!(back, PostConfig::default());
    }
}
