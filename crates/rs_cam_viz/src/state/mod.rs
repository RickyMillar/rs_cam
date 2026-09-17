pub mod freshness;
pub mod history;
pub mod job;
pub mod multitool_planner;
pub mod overlays;
pub mod panels;
pub mod rest_dependency;
pub mod runtime;
pub mod selection;
pub mod simulation;
pub mod stale;
pub mod toolpath;
pub mod viewport;
pub mod wizard;

use history::UndoHistory;
use overlays::OverlayPanelState;
use panels::PanelDrafts;
use runtime::GuiState;
use selection::Selection;
use simulation::SimulationState;
use viewport::ViewportState;

/// Which top-level workspace the user is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Workspace {
    /// Setup definition: stock orientation, datum, workholding, fixtures.
    Setup,
    /// Toolpath authoring — parameters, feeds, strategies.
    Toolpaths,
    /// Verification — material removal, collisions, cycle time, safety.
    Simulation,
    /// Job-readiness dashboard — "is this safe to cut?" in one place (W3.8).
    Readiness,
}

impl Workspace {
    /// Every workspace, in the order the operator meets them.
    ///
    /// ONE list, on the `ui/overlays/registry.rs` pattern: the switcher bar,
    /// the Workspace menu, the MCP key round-trip and the completeness sentry
    /// all read this, so a workspace cannot exist on one surface and be
    /// missing from another. It was Readiness that went missing — present on
    /// the tab bar since W3.8 and never added to the menu, which listed three
    /// of four by hand (G-WSMENU, 2026-09-10).
    pub const ALL: [Workspace; 4] = [
        Workspace::Setup,
        Workspace::Toolpaths,
        Workspace::Simulation,
        Workspace::Readiness,
    ];

    /// This workspace's position in [`Workspace::ALL`].
    ///
    /// The match is exhaustive, so a new variant does not COMPILE until it
    /// is given a position, and the sentry then fails unless that position
    /// is a real, unique slot in `ALL`. That pair is what makes `ALL`
    /// complete rather than merely long enough.
    #[must_use]
    pub fn order(self) -> usize {
        match self {
            Workspace::Setup => 0,
            Workspace::Toolpaths => 1,
            Workspace::Simulation => 2,
            Workspace::Readiness => 3,
        }
    }

    /// The tab and menu label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Workspace::Setup => "Setup",
            Workspace::Toolpaths => "Toolpaths",
            Workspace::Simulation => "Simulation",
            Workspace::Readiness => "Readiness",
        }
    }

    /// The one-line hint the switcher bar prints for the active workspace.
    #[must_use]
    pub fn hint(self) -> &'static str {
        match self {
            Workspace::Setup => "Stock, orientation, workholding",
            Workspace::Toolpaths => "Operations, tools, generation",
            Workspace::Simulation => "Verify, animate, export",
            Workspace::Readiness => "Is this safe to cut?",
        }
    }
}

/// The sentence describing an open project that has edits which are not on
/// disk, or `None` when there is nothing unsaved to lose.
///
/// G-OPENGUARD (F1.12). Anything that REPLACES the open project — quitting,
/// File > Open, MCP `load_project` — discards those edits. The GUI asks a
/// human through the unsaved-changes dialog; MCP has nobody to ask, so it
/// turns this sentence into a refusal and requires `discard_unsaved: true`
/// to proceed.
///
/// It names what would be lost rather than only that something would be. A
/// project that has NEVER been saved loses everything in it, so the
/// sentence counts what is there; one with a file on disk loses the changes
/// made since, and the sentence names the file they are missing from. It
/// deliberately does not invent a count of "changes": `GuiState::edit_counter`
/// counts `mark_edited` calls, not distinct differences, and quoting it
/// would be a number that means nothing to the operator.
pub fn unsaved_project_summary(state: &AppState) -> Option<String> {
    if !state.gui.dirty {
        return None;
    }
    let name = state.session.name();
    let label = if name.trim().is_empty() {
        "the open project".to_owned()
    } else {
        format!("the open project '{name}'")
    };
    Some(match state.gui.file_path.as_ref() {
        Some(path) => format!("{label} has changes that are not in {}", path.display()),
        None => format!(
            "{label} has never been saved, so everything in it would be lost              -- {} setups, {} toolpaths, {} tools",
            state.session.setup_count(),
            state.session.toolpath_count(),
            state.session.tools().len(),
        ),
    })
}

/// Top-level application state. Single source of truth.
pub struct AppState {
    pub workspace: Workspace,
    /// Unified project session — single source of truth for CAM data.
    pub session: rs_cam_core::session::ProjectSession,
    /// GUI-only runtime overlay (dirty flag, per-toolpath display state, datum config).
    pub gui: GuiState,
    pub selection: Selection,
    pub viewport: ViewportState,
    /// The viewport Overlays panel's own state (P6). The overlay FLAGS live
    /// on [`ViewportState`]; this is only the panel's shape plus the
    /// per-workspace default bookkeeping.
    pub overlays: OverlayPanelState,
    pub simulation: SimulationState,
    pub history: UndoHistory,
    /// UI-09: the panel drafts that outlive a frame — the text an operator
    /// typed and the status line a panel printed. They used to sit in egui
    /// temporary memory, where MCP and the integration harness could not
    /// read them and the GRBL dump was re-parsed every frame.
    pub panels: PanelDrafts,
    /// Show pre-flight checklist modal before export.
    pub show_preflight: bool,
    /// Show keyboard shortcuts reference window.
    pub show_shortcuts: bool,
    /// Show the multi-step Export Wizard. Persistent settings live on
    /// `gui.wizard`; this flag and `wizard_active_step` are
    /// transient UI state.
    pub show_export_wizard: bool,
    /// 0-indexed currently visible wizard step. Initialised from
    /// `gui.wizard.last_step_visited` when the wizard opens so
    /// the user resumes where they left off.
    pub wizard_active_step: u8,
    /// Cached state of the per-toolpath Optimize modal. `None` when
    /// closed. The optimizer is expensive (~1-2 min per toolpath at
    /// the Stage 0/1/2 settings), so unlike the suggest modal we
    /// cannot recompute every frame — the outcome is captured here on
    /// open and rendered every frame from this cache.
    pub optimize_modal: Option<OptimizeModalState>,
    /// Cached project-level Optimize rollup (U3). `None` until the
    /// user clicks the toolbar Optimize-project button. While the
    /// worker is running, status is `Loading`; once the result lands,
    /// the rollup view renders the report. Mirrors the per-toolpath
    /// modal's lifecycle so the UI shapes stay consistent.
    pub optimize_project: Option<OptimizeProjectState>,
    /// The Optimize run in flight, or `None` when none is.
    ///
    /// It was a bare `is_optimizing: bool` until WP24. The flag once meant
    /// "the worker thread HOLDS the session", and the main thread then
    /// rendered a full-screen placeholder. WP14b gave every route a clone
    /// or a handle, so `state.session` is the real project throughout, and
    /// WP24 deleted the placeholder on the operator's ruling of
    /// 2026-09-13 (§30 item 3). What is left is a POLICY (§28 ruling 8):
    /// one Optimize run at a time, refused with a toast.
    ///
    /// The run is now a value rather than a flag, because the workspace
    /// bar's progress row has to NAME what is running and offer a cancel
    /// for that one submit. [`AppState::is_optimizing`] answers the
    /// policy question the three refusal sites ask.
    ///
    /// One writer stamps it — `AppController::submit_gui_job` for the two
    /// `Job` rows and `AppController::open_optimize_project` for the
    /// rollup — and two clear it:
    /// `AppController::deliver_gui_job` for the two `Job` rows, and
    /// `AppController::handle_optimize_result` for the project rollup.
    pub optimize_run: Option<OptimizeRun>,
    /// Set after the user clicks Apply selected on the Optimize-project
    /// rollup. Holds the toolpath ids that need to finish regenerating
    /// before we kick the reconciliation sim. Empty otherwise.
    pub pending_reconciliation_for_ids: Vec<rs_cam_core::ToolpathId>,
    /// Toolpath id of a per-TP Optimize candidate just applied via
    /// `apply_optimize_candidate`. When the corresponding regen lands,
    /// the drain handler auto-triggers a full project sim so the
    /// freshly-applied params are verified against live simulation
    /// without the user having to click Run Simulation manually
    /// (Roadmap F.2). `None` when no per-TP Apply is in flight.
    pub pending_apply_resim: Option<usize>,
    /// Cached state of the redesigned Feeds & Speeds modal. `None`
    /// when closed. Built on open by `explain_for_operation`; the
    /// modal re-derives `FeedsExplain` every frame from session state
    /// so live param edits are reflected without an extra refresh
    /// path.
    pub feeds_modal: Option<FeedsModalState>,
    /// Project-scope feeds state — the rollup over every toolpath.
    ///
    /// It sits beside `feeds_modal` rather than inside it (DC5a). The two
    /// answer at different scopes, so neither owns the other's state.
    pub project_feeds: ProjectFeedsState,
    /// Cached state of the Tool Library management modal. `None` when
    /// closed. Holds a snapshot of every catalog loaded on open; the
    /// controller refreshes the snapshot after any mutation so the
    /// modal can render without per-frame disk I/O.
    pub tool_library_modal: Option<ToolLibraryModalState>,
    /// Whether the Machine Library modal is open. Unlike the tool library
    /// (which caches a catalog snapshot), the machine modal reads the
    /// one-file-per-machine library directly each frame (cheap), so a bool
    /// is enough.
    pub machine_library_open: bool,
    /// Multi-tool finishing planner dialog (Phase U). `None` until it is
    /// first opened, and then **never dropped**: closing sets
    /// `open = false` and keeps the ladder, the dials and any held preview,
    /// because rejecting a preview has to re-open onto the same dialog
    /// (`ORCHESTRATION_PLAN.md` §3.1) rather than a fresh one.
    pub multitool_planner: Option<multitool_planner::MultitoolPlannerState>,
    /// WP6 — work a draw site owes the frame loop after it applies a
    /// command.
    pub panel_side_effects: PanelSideEffects,
}

/// Work a properties panel owes the frame loop.
///
/// A draw site holds an [`AppState`] and nothing else. The three things
/// below live on the controller, so a panel cannot do them itself. It
/// raises the flag; `AppController::discharge_panel_side_effects` runs
/// the work once per frame.
///
/// The first two were carried by the five post-write `AppEvent`s that
/// plan §14 ruling 3 deletes. The invalidation half of those events
/// moved into the command rows; this is the half that did NOT — it is
/// view work and project bookkeeping, not a core rule. WP19 added the
/// third for the same reason: the core answer says the session dropped
/// its simulation, and clearing the view's is controller work.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PanelSideEffects {
    /// The viewport's GPU buffers read something the panel moved — the
    /// stock box, a fixture, a keep-out zone, or a height plane.
    pub upload: bool,
    /// The stock's alignment pins or flip axis moved, so the
    /// auto-generated pin-drill operation must be created, updated or
    /// removed.
    pub pin_drill_sync: bool,
    /// The session dropped its simulation, and the viewport still holds
    /// one (WP19).
    ///
    /// `Effects::simulation_cleared` reports the drop. Only the
    /// controller can run `invalidate_simulation`, so a draw site
    /// raises this and the frame loop clears the view.
    pub invalidate_simulation: bool,
}

impl PanelSideEffects {
    /// Take the flags and clear them.
    pub fn take(&mut self) -> Self {
        std::mem::take(self)
    }
}

/// Persistent state for the Tool Library modal. The `catalogs` snapshot
/// is loaded by the controller when the modal opens and re-loaded after
/// every mutating action; the modal renders entirely from it. Ephemeral
/// view state (selection, filter, edit drafts) lives in egui temp memory.
#[derive(Debug, Clone)]
pub struct ToolLibraryModalState {
    pub catalogs: Vec<(String, rs_cam_core::io::tool_library::ToolCatalog)>,
}

/// Persistent state for the Feeds & Speeds modal. Carries the focused
/// toolpath plus per-modal UI state (active mode, hover state, "what
/// if" drag, etc.). The underlying recommendation + chart data is
/// derived fresh from the session each frame.
#[derive(Debug, Clone)]
pub struct FeedsModalState {
    pub toolpath_id: rs_cam_core::ToolpathId,
    /// Phase 3 — drag-to-explore on the feed-RPM nomogram. `Some` while
    /// the user is dragging the operating point.
    pub explore: Option<NomogramExplore>,
}

/// Project-scope feeds state — the rollup over EVERY toolpath.
///
/// DC5a moved these fields off [`FeedsModalState`]. They were the cause of
/// the defect, not a symptom of it: a project-wide sort order, selection set
/// and chart toggle lived on the PER-OPERATION modal, so the rollup's
/// controls only functioned while that modal was open. One container holds
/// one scope, and that rule applies to the state as much as to the drawing.
///
/// The rollup lives in the Readiness workspace, which already answers
/// project-wide questions.
#[derive(Debug, Clone, Default)]
pub struct ProjectFeedsState {
    /// True while the rollup detail is on screen. The Readiness check row
    /// opens it; the window's own close button shuts it.
    pub open: bool,
    /// Sortable column for the per-toolpath table.
    pub sort: ProjectFeedsSort,
    /// Toolpath ids whose row checkbox is ticked. Empty means nothing is
    /// selected, which disables Apply.
    ///
    /// **This set is SEEDED when the rollup opens**, from every enabled
    /// toolpath. Before DC5a the seed ran as a side effect of opening the
    /// per-operation feeds modal. An unseeded rollup opens with every row
    /// unticked, which reads as "nothing to report" on a project-scope
    /// surface — an abstention drawn as a pass.
    pub selected: std::collections::BTreeSet<rs_cam_core::ToolpathId>,
    /// Toggle for the scatter overlay.
    pub show_scatter: bool,
}

/// Local UI state for the drag-to-explore interaction on Chart C.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NomogramExplore {
    /// Currently dragged (rpm, feed) operating point.
    pub rpm: f64,
    pub feed_mm_min: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectFeedsSort {
    /// Project order. The default, and what the table shipped with.
    #[default]
    Index,
    Speedup,
    Name,
}

/// Persistent state for the per-toolpath Optimize modal. Carries the
/// toolpath being optimized plus the cached outcome (or its Loading /
/// Failed states for the worker-thread integration that lands in U3).
#[derive(Debug, Clone)]
pub struct OptimizeModalState {
    pub toolpath_id: rs_cam_core::ToolpathId,
    pub status: OptimizeRunStatus,
}

/// Lifecycle of one Optimize run as the modal sees it. The lane
/// driver moves `Loading -> Ready` (or `Failed`) as the worker
/// completes; the modal renders the right view per status.
#[derive(Debug, Clone)]
pub enum OptimizeRunStatus {
    /// Optimizer is running on a worker thread. The modal shows a
    /// progress strip and a Cancel button.
    Loading,
    /// Optimizer finished. The outcome is the source of truth for
    /// every row in the modal's candidate table.
    Ready(rs_cam_core::tool_load::optimize::OptimizeOutcome),
    /// Optimizer errored out. String is the diagnostic for the user.
    Failed(String),
}

/// The Optimize run in flight, as the progress row sees it (WP24).
///
/// The workspace bar draws one row per run: the label this carries, the
/// elapsed time, and a Cancel that arms THIS submit. The row replaces the
/// full-screen placeholder, so it is the only thing that tells the
/// operator a run is under way.
///
/// Since WP29 the `Toolpath` kind also carries the search's own progress,
/// so the row names the rung of the ladder and the candidate inside it.
/// The two other kinds run no candidate ladder and carry `None`. This doc
/// used to read "DEFERRED: no `phase` field … a phase field would have no
/// writer"; plan §33 gave the search a writer.
#[derive(Debug, Clone)]
pub struct OptimizeRun {
    pub kind: OptimizeRunKind,
    /// `Some` for the two `Job` rows; `None` for the rollup, which rides
    /// `ComputeLane::Optimize` and carries no `Job` id.
    pub job_id: Option<crate::compute::JobRequestId>,
    pub started_at: std::time::Instant,
    /// Set when a cancel was armed. The row then reads "cancelling".
    ///
    /// It does NOT clear the run — the drain does. A cancelled Optimize
    /// still returns a partial outcome, and clearing early would let the
    /// first submit's drain wipe a second submit's run.
    pub cancel_requested: bool,
    /// What the candidate search publishes about itself (WP29).
    ///
    /// `Some` on the `Toolpath` kind, whose handle carries the same `Arc`.
    /// `None` on the rollup and on the tier-map preview, which run no
    /// candidate ladder — so `None` means THIS RUN WALKS NO LADDER, and
    /// the row shows the elapsed seconds alone.
    pub progress: Option<std::sync::Arc<rs_cam_core::tool_load::optimize::OptimizeProgress>>,
}

/// How one rung of the search ladder stands, for the Optimize window's
/// list (WP29).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizeStageMark {
    /// The search passed this rung.
    Done,
    /// The search is on this rung now.
    Current,
    /// The search has not reached this rung.
    Pending,
}

/// One row of the Optimize window's rung list (WP29).
///
/// A pure value, so a test reads the window's list without an egui pass.
/// The draw picks the glyph for the [`OptimizeStageMark`]; this type holds
/// no colour and no glyph.
///
/// Stays `pub`: it is the return element of `pub fn OptimizeRun::stage_rows`,
/// so a crate-private form raises `private_interfaces` (S29, 2026-09-16).
#[derive(Debug, Clone)]
pub struct OptimizeStageRow {
    /// Where the search stands on this rung.
    pub mark: OptimizeStageMark,
    /// The rung's name, with its place on the ladder.
    pub name: String,
    /// The rung's candidate count. `None` means NOT ANNOUNCED — a rung's
    /// total is unknown until that rung starts, so an unannounced rung
    /// shows no count rather than a zero.
    pub count: Option<String>,
}

impl OptimizeRun {
    /// The sentence the progress row shows (WP29).
    ///
    /// **One builder for the row and for the sentry.** The row used to
    /// `format!` its own text, so no test could read the sentence the
    /// operator reads. `ui/workspace_bar.rs` now calls this.
    ///
    /// There is NO time-left figure, and this function holds no estimate.
    /// The whole-run candidate total is not known up front, and a refine
    /// candidate simulates at a finer cell than a grid candidate, so one
    /// mean second per candidate mixes two populations.
    #[must_use]
    pub fn progress_text(&self, session: &rs_cam_core::session::ProjectSession) -> String {
        let label = self.kind.label(session);
        let seconds = self.started_at.elapsed().as_secs();
        let stage = self.stage_phrase();
        match (self.cancel_requested, stage) {
            (true, Some(stage)) => format!("{label} — cancelling · {stage} · {seconds} s"),
            // The WP24 sentence, kept byte-for-byte for a run that walks
            // no ladder.
            (true, None) => format!("{label} — cancelling ({seconds} s)"),
            (false, Some(stage)) => format!("{label} — {stage} · {seconds} s"),
            (false, None) => format!("{label} — {seconds} s"),
        }
    }

    /// The rung list the Optimize window draws (WP29).
    ///
    /// Empty for a run that walks no candidate ladder, so the rollup and
    /// the tier-map preview list nothing rather than three pending rungs.
    #[must_use]
    pub fn stage_rows(&self) -> Vec<OptimizeStageRow> {
        use rs_cam_core::tool_load::optimize::SearchPhase;

        let Some(progress) = self.progress.as_ref() else {
            return Vec::new();
        };
        let snapshot = progress.snapshot();
        // A settled run claims no rung, so the mark falls back to the
        // high-water index. That keeps a cancelled run's window honest:
        // the rungs it walked still read done.
        let announced = snapshot.phase_index.max(snapshot.high_water_index);
        let rungs = snapshot.phase_count;
        SearchPhase::ALL
            .into_iter()
            .map(|phase| {
                let index = phase.index_from_one();
                let mark = if index == snapshot.phase_index {
                    OptimizeStageMark::Current
                } else if index <= announced {
                    OptimizeStageMark::Done
                } else {
                    OptimizeStageMark::Pending
                };
                let count = if index > announced {
                    None
                } else {
                    let (entered, formed) = progress.rung(phase);
                    if formed == 0 {
                        Some("no candidates".to_owned())
                    } else {
                        Some(format!("{entered} / {formed}"))
                    }
                };
                OptimizeStageRow {
                    mark,
                    name: format!("Stage {index}/{rungs} — {}", phase.label()),
                    count,
                }
            })
            .collect()
    }

    /// The row's stage half: `stage 2/3 · candidate 3/8`.
    ///
    /// `None` when the run walks no ladder, and `None` again when the
    /// ladder has settled — a finished run must not leave the row claiming
    /// a rung.
    fn stage_phrase(&self) -> Option<String> {
        let snapshot = self.progress.as_ref()?.snapshot();
        let _ = snapshot.phase?;
        let index = snapshot.phase_index;
        let count = snapshot.phase_count;
        if snapshot.candidate_total == 0 {
            return Some(format!("stage {index}/{count} · no candidates"));
        }
        let candidate = snapshot.candidate;
        let total = snapshot.candidate_total;
        let phrase = format!("stage {index}/{count} · candidate {candidate}/{total}");
        Some(phrase)
    }
}

/// Which of the three runs is in flight.
///
/// They are three submits on two lanes, and the field covered all three
/// before WP24 named them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizeRunKind {
    /// `AppController::open_optimize_modal` — the `optimize_toolpath`
    /// `Job` row.
    Toolpath {
        toolpath_id: rs_cam_core::ToolpathId,
    },
    /// `AppController::open_optimize_project` — the rollup on
    /// `ComputeLane::Optimize`.
    Project,
    /// `AppController::request_multitool_preview` — the
    /// `preview_tier_map` `Job` row.
    MultitoolPreview,
}

impl OptimizeRunKind {
    /// Name the run for the progress row and for a refusal toast.
    ///
    /// One function serves both, so the sentence the operator reads when
    /// a second request is refused names the run the row is showing.
    #[must_use]
    pub fn label(&self, session: &rs_cam_core::session::ProjectSession) -> String {
        match self {
            Self::Toolpath { toolpath_id } => {
                let config = session
                    .toolpath_configs()
                    .iter()
                    .find(|tc| tc.id == *toolpath_id);
                match config {
                    Some(tc) => format!("Optimize {}", tc.name),
                    None => format!("Optimize toolpath {toolpath_id}"),
                }
            }
            Self::Project => "Optimize project".to_owned(),
            Self::MultitoolPreview => "Tier-map preview".to_owned(),
        }
    }
}

/// Persistent state for the project-level Optimize rollup (U3).
/// Mirrors `OptimizeModalState`'s lifecycle but without a single
/// toolpath_id — the rollup spans every enabled toolpath.
#[derive(Debug, Clone)]
pub struct OptimizeProjectState {
    pub status: OptimizeProjectStatus,
    /// Per-row checkbox state for batch Apply. Index aligned with
    /// `ProjectOptimizeReport::per_toolpath`. Defaults to true on the
    /// rows whose outcome has a recommended candidate; the user can
    /// flip individual rows before clicking Apply selected.
    pub row_selected: Vec<bool>,
}

#[derive(Debug, Clone)]
pub enum OptimizeProjectStatus {
    /// Worker is running. Rollup view shows progress + cancel.
    Loading,
    /// Worker finished. Render the rollup with bottleneck callout
    /// and the per-toolpath rows.
    Ready(rs_cam_core::tool_load::optimize::ProjectOptimizeReport),
    /// User clicked Apply selected; the project sim is now running
    /// end-to-end with the applied params. Rollup stays visible with
    /// the candidate-isolated values dimmed; reconciled column lights
    /// up when the sim completes.
    Reconciling(rs_cam_core::tool_load::optimize::ProjectOptimizeReport),
    /// Project sim completed; per-row `reconciled_cycle_time_s` /
    /// `reconciled_verdict` are populated. Rows whose reconciled
    /// values disagree with candidate-isolated values are flagged.
    Reconciled(rs_cam_core::tool_load::optimize::ProjectOptimizeReport),
    /// Worker failed. String is the diagnostic for the user.
    Failed(String),
}

impl AppState {
    pub fn new() -> Self {
        Self {
            workspace: Workspace::Toolpaths,
            session: rs_cam_core::session::ProjectSession::new_empty(),
            gui: GuiState::new(),
            selection: Selection::None,
            viewport: ViewportState::new(),
            overlays: OverlayPanelState::new(),
            simulation: SimulationState::new(),
            history: UndoHistory::new(),
            panels: PanelDrafts::default(),
            show_preflight: false,
            show_shortcuts: false,
            show_export_wizard: false,
            wizard_active_step: 0,
            optimize_modal: None,
            optimize_project: None,
            optimize_run: None,
            pending_reconciliation_for_ids: Vec::new(),
            pending_apply_resim: None,
            feeds_modal: None,
            project_feeds: ProjectFeedsState::default(),
            tool_library_modal: None,
            machine_library_open: false,
            multitool_planner: None,
            panel_side_effects: PanelSideEffects::default(),
        }
    }

    /// Index of the setup whose local frame the viewport is displaying —
    /// the selection's setup, or the FIRST setup when nothing
    /// setup-scoped is selected (the same rule `ui::setup_panel` and the
    /// GPU upload pass apply from the selection). `None` only for a
    /// project with no setups.
    ///
    /// The tier preview gates on this: a preview describes ONE setup's
    /// plan, and in any other setup's display frame it is not merely
    /// irrelevant but wrongly shifted — the operator-observed defect was
    /// the front setup's tier map floating offset beside the flipped
    /// back-setup stock.
    pub fn active_setup_index(&self) -> Option<usize> {
        let setups = self.session.list_setups();
        let by_id =
            |id: job::SetupId| -> Option<usize> { setups.iter().position(|s| s.id == id.0) };
        match &self.selection {
            Selection::Setup(id) | Selection::Fixture(id, _) | Selection::KeepOut(id, _) => {
                by_id(*id)
            }
            Selection::Toolpath(tp_id) => self.session.setup_of_toolpath_id(*tp_id),
            _ => {
                if setups.is_empty() {
                    None
                } else {
                    Some(0)
                }
            }
        }
    }

    /// Is an Optimize run in flight?
    ///
    /// The policy question (§28 ruling 8): one Optimize run at a time. The
    /// three submit sites ask it and refuse with a toast, and the two menu
    /// entries disable on it. It is NOT a drawing question any more — WP24
    /// deleted the full-screen placeholder that made it one.
    #[must_use]
    pub fn is_optimizing(&self) -> bool {
        self.optimize_run.is_some()
    }

    /// Modal exclusivity (density pass Batch 2): every modal-open path
    /// calls this first, so opening one modal closes the others — the
    /// 2026-06-11 capture sweep produced a 3-deep stack (Optimize
    /// spinner over Tool Library over a feeds modal). A *running*
    /// Optimize modal survives: it is a progress surface for an
    /// expensive in-flight search, and closing it here would discard
    /// the user's run; settled Optimize outcomes close like the rest.
    pub fn close_modals_for_exclusivity(&mut self) {
        self.feeds_modal = None;
        self.tool_library_modal = None;
        self.machine_library_open = false;
        self.show_export_wizard = false;
        self.show_preflight = false;
        self.show_shortcuts = false;
        if !self.is_optimizing() {
            self.optimize_modal = None;
            self.optimize_project = None;
            // Closed, not discarded — see the field doc. A planner whose
            // preview is in flight is spared for the same reason a running
            // Optimize is: closing it would throw away a walk the operator
            // is waiting on.
            if let Some(planner) = self.multitool_planner.as_mut() {
                planner.open = false;
            }
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn feeds_modal_fixture() -> FeedsModalState {
        FeedsModalState {
            toolpath_id: rs_cam_core::ToolpathId(0),
            explore: None,
        }
    }

    /// Density pass Batch 2 — opening any modal closes the others. The
    /// 2026-06-11 capture sweep stacked Optimize over Tool Library over
    /// a feeds modal; exclusivity makes that state unrepresentable.
    #[test]
    fn close_modals_for_exclusivity_closes_settled_modals() {
        let mut state = AppState::new();
        state.feeds_modal = Some(feeds_modal_fixture());
        state.tool_library_modal = Some(ToolLibraryModalState { catalogs: vec![] });
        state.show_export_wizard = true;
        state.show_preflight = true;
        state.optimize_project = Some(OptimizeProjectState {
            status: OptimizeProjectStatus::Loading,
            row_selected: Vec::new(),
        });

        state.close_modals_for_exclusivity();

        assert!(state.feeds_modal.is_none());
        assert!(state.tool_library_modal.is_none());
        assert!(!state.show_export_wizard);
        assert!(!state.show_preflight);
        assert!(state.optimize_project.is_none(), "settled optimize closes");
    }

    /// A *running* Optimize modal must survive exclusivity — closing it
    /// would silently discard an expensive in-flight search.
    #[test]
    fn close_modals_for_exclusivity_spares_running_optimize() {
        let mut state = AppState::new();
        state.optimize_run = Some(OptimizeRun {
            kind: OptimizeRunKind::Toolpath {
                toolpath_id: rs_cam_core::ToolpathId(0),
            },
            job_id: Some(crate::compute::JobRequestId(0)),
            started_at: std::time::Instant::now(),
            cancel_requested: false,
            progress: None,
        });
        state.optimize_modal = Some(OptimizeModalState {
            toolpath_id: rs_cam_core::ToolpathId(0),
            status: OptimizeRunStatus::Loading,
        });
        state.feeds_modal = Some(feeds_modal_fixture());

        state.close_modals_for_exclusivity();

        assert!(state.optimize_modal.is_some(), "running optimize survives");
        assert!(state.feeds_modal.is_none(), "others still close");
    }
}
