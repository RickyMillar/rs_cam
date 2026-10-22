pub mod app_settings;
pub mod freshness;
pub mod history;
pub mod job;
pub mod multitool_planner;
pub mod overlays;
pub mod panels;
pub mod pick;
pub mod pin_view;
pub mod runtime;
pub mod selection;
pub mod selection_highlight;
pub mod shoe_plan;
pub mod simulation;
pub mod stale;
pub mod tab_edit;
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool is one independent window or panel that is open or closed"
)]
pub struct AppState {
    pub workspace: Workspace,
    /// Unified project session — single source of truth for CAM data.
    ///
    /// F096 (W9b): the field is private. Read it with [`Self::session`].
    /// Change it with [`Self::apply`], the one door that applies a
    /// `Command` and mirrors its `Effects`. Replace it with
    /// [`Self::replace_session`]. A surface outside `state/` cannot take
    /// `&mut` of the session, so it cannot skip the mirror.
    session: rs_cam_core::session::ProjectSession,
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
    /// The GUI job in flight, or `None` when none is.
    ///
    /// It was a bare `bool` until WP24. The run is a value
    /// rather than a flag, because the workspace bar's progress row has to
    /// NAME what is running and offer a cancel for that one submit.
    /// [`AppState::is_job_running`] answers the policy question: one GUI
    /// job at a time, refused with a toast.
    ///
    /// One writer stamps it — `AppController::submit_gui_job` — and one
    /// clears it: `AppController::deliver_gui_job`.
    pub job_run: Option<JobRun>,
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
    /// The File ▸ Preferences window and its whole draft: the category,
    /// every choice and every typed text. `None` when the window is closed.
    /// The controller builds it on open (`AppEvent::OpenPreferences`).
    pub preferences: Option<crate::ui::preferences::PreferencesState>,
    /// The app settings that the running GUI uses: the settings file as it
    /// was at start, then as File ▸ Preferences last applied it. Not part
    /// of the project; never saved with it. See `state/app_settings.rs`.
    pub app_settings: rs_cam_core::settings::AppSettings,
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

/// The session to read beside every other [`AppState`] field to write.
///
/// [`AppState::split_mut`] builds it. A draw site that reads the session
/// and writes a view field in one call takes it, because
/// [`AppState::session`] borrows the whole state. The session part is a
/// shared reference, so the split cannot write the session either (F096).
pub struct StateParts<'a> {
    pub session: &'a rs_cam_core::session::ProjectSession,
    pub workspace: &'a mut Workspace,
    pub gui: &'a mut GuiState,
    pub selection: &'a mut Selection,
    pub viewport: &'a mut ViewportState,
    pub overlays: &'a mut OverlayPanelState,
    pub simulation: &'a mut SimulationState,
    pub history: &'a mut UndoHistory,
    pub panels: &'a mut PanelDrafts,
    pub show_preflight: &'a mut bool,
    pub show_shortcuts: &'a mut bool,
    pub show_export_wizard: &'a mut bool,
    pub wizard_active_step: &'a mut u8,
    pub job_run: &'a mut Option<JobRun>,
    pub feeds_modal: &'a mut Option<FeedsModalState>,
    pub project_feeds: &'a mut ProjectFeedsState,
    pub tool_library_modal: &'a mut Option<ToolLibraryModalState>,
    pub machine_library_open: &'a mut bool,
    pub preferences: &'a mut Option<crate::ui::preferences::PreferencesState>,
    pub app_settings: &'a mut rs_cam_core::settings::AppSettings,
    pub multitool_planner: &'a mut Option<multitool_planner::MultitoolPlannerState>,
    pub panel_side_effects: &'a mut PanelSideEffects,
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
    /// controller can run `invalidate_simulation`, so
    /// [`AppState::apply`] raises this and the frame loop clears the
    /// view. `AppController::apply` clears the view at once and lowers
    /// the flag (F096).
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

/// The GUI job in flight, as the progress row sees it (WP24).
///
/// The workspace bar draws one row per run: the label this carries, the
/// elapsed time, and a Cancel that arms THIS submit. The row replaces the
/// full-screen placeholder, so it is the only thing that tells the
/// operator a run is under way.
#[derive(Debug, Clone)]
pub struct JobRun {
    pub kind: JobRunKind,
    /// The id of the `Job` submit.
    pub job_id: crate::compute::JobRequestId,
    pub started_at: std::time::Instant,
    /// Set when a cancel was armed. The row then reads "cancelling".
    ///
    /// It does NOT clear the run — the drain does. Clearing early would
    /// let the first submit's drain wipe a second submit's run.
    pub cancel_requested: bool,
}

impl JobRun {
    /// The sentence the progress row shows (WP29).
    ///
    /// **One builder for the row and for the sentry.** `ui/workspace_bar.rs`
    /// calls this. There is NO time-left figure.
    #[must_use]
    pub fn progress_text(&self) -> String {
        let label = self.kind.label();
        let seconds = self.started_at.elapsed().as_secs();
        if self.cancel_requested {
            format!("{label} — cancelling ({seconds} s)")
        } else {
            format!("{label} — {seconds} s")
        }
    }
}

/// Which GUI job is in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobRunKind {
    /// `AppController::request_multitool_preview` — the
    /// `preview_tier_map` `Job` row.
    MultitoolPreview,
    /// `AppController::request_model_faces` — the `build_model_faces`
    /// `Job` row (geometry selection P4.7).
    ModelFaces,
}

impl JobRunKind {
    /// Name the run for the progress row and for a refusal toast.
    ///
    /// One function serves both, so the sentence the operator reads when
    /// a second request is refused names the run the row is showing.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::MultitoolPreview => "Tier-map preview",
            Self::ModelFaces => "Computing faces",
        }
    }
}

impl AppState {
    /// The project session, to read.
    ///
    /// The only way to change it is [`Self::apply`] (F096).
    #[must_use]
    pub fn session(&self) -> &rs_cam_core::session::ProjectSession {
        &self.session
    }

    /// Apply one command to the session. THE one write door of the GUI (F096).
    ///
    /// The door mirrors both halves of the core answer:
    ///
    /// - `Effects::stale` goes to [`stale::stamp_stale`], so every
    ///   toolpath that core dropped shows stale on its card.
    /// - `Effects::simulation_cleared` raises
    ///   `panel_side_effects.invalidate_simulation`. The viewport
    ///   simulation lives partly on the controller, so the controller
    ///   clears it: at once in `AppController::apply`, or at the end of
    ///   the frame in `discharge_panel_side_effects`.
    ///
    /// A refusal goes to the log here (F056) and returns to the caller.
    /// The caller decides what the operator sees. A caller must read the
    /// answer before it marks the project edited.
    #[must_use = "a refused command changed nothing; read the answer before you mark the project edited"]
    pub fn apply(
        &mut self,
        command: rs_cam_core::session::Command,
    ) -> Result<rs_cam_core::session::Effects, rs_cam_core::session::SessionError> {
        match self.session.apply(command) {
            Ok(effects) => {
                stale::stamp_stale(self, &effects.stale);
                if effects.simulation_cleared {
                    self.panel_side_effects.invalidate_simulation = true;
                }
                Ok(effects)
            }
            Err(error) => {
                tracing::warn!("the session refused a command: {error}");
                Err(error)
            }
        }
    }

    /// The session to read and every other field to write, as disjoint
    /// borrows. See [`StateParts`].
    pub fn split_mut(&mut self) -> StateParts<'_> {
        let Self {
            session,
            workspace,
            gui,
            selection,
            viewport,
            overlays,
            simulation,
            history,
            panels,
            show_preflight,
            show_shortcuts,
            show_export_wizard,
            wizard_active_step,
            job_run,
            feeds_modal,
            project_feeds,
            tool_library_modal,
            machine_library_open,
            preferences,
            app_settings,
            multitool_planner,
            panel_side_effects,
        } = self;
        StateParts {
            session,
            workspace,
            gui,
            selection,
            viewport,
            overlays,
            simulation,
            history,
            panels,
            show_preflight,
            show_shortcuts,
            show_export_wizard,
            wizard_active_step,
            job_run,
            feeds_modal,
            project_feeds,
            tool_library_modal,
            machine_library_open,
            preferences,
            app_settings,
            multitool_planner,
            panel_side_effects,
        }
    }

    /// Start one core job on the session (`ProjectSession::start`).
    ///
    /// A job start records the job in the session and changes no
    /// project data, so it returns no `Effects`.
    pub fn start_job(
        &mut self,
        job: rs_cam_core::session::Job,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<rs_cam_core::session::JobHandle, rs_cam_core::session::SessionError> {
        self.session.start(job, cancel)
    }

    /// Put a different project in place of the open one.
    ///
    /// The project open and new-project doors call it, and the test
    /// fixtures call it with a built session. It changes the session
    /// only: the caller resets the GUI state that the old project owned.
    pub fn replace_session(&mut self, session: rs_cam_core::session::ProjectSession) {
        self.session = session;
    }

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
            job_run: None,
            feeds_modal: None,
            project_feeds: ProjectFeedsState::default(),
            tool_library_modal: None,
            machine_library_open: false,
            preferences: None,
            app_settings: rs_cam_core::settings::AppSettings::default(),
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
        let by_id = |id: job::SetupId| -> Option<usize> { setups.iter().position(|s| s.id == id) };
        match &self.selection {
            Selection::Setup(id) | Selection::Obstacle(id, _) => by_id(*id),
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

    /// Where the simulation stands. THE one answer, for every surface.
    ///
    /// W4, G-FRESHNESSDISAGREE. Each reader used to build its own from
    /// `SimulationState::is_stale(gui.edit_counter)`, and several paired it
    /// with a second `has_results()` read. The counter is not the project's
    /// truth about a simulation; `ProjectSession::simulation_result()` is.
    #[must_use]
    pub fn simulation_freshness(&self) -> freshness::SimFreshness {
        freshness::simulation_freshness(&self.session, &self.simulation)
    }

    /// Whether a reader must not present the simulation as current evidence.
    ///
    /// A thin predicate over [`Self::simulation_freshness`]. A reader that
    /// needs to tell "nothing to show" from "showing something old" asks
    /// for the arm instead.
    #[must_use]
    pub fn simulation_is_stale(&self) -> bool {
        self.simulation_freshness().is_stale()
    }

    /// Why the simulation is stale, or `None` when it is not.
    ///
    /// A thin read over [`Self::simulation_freshness`]. Every stale label
    /// reads it, so the label names the real cause.
    #[must_use]
    pub fn simulation_stale_reason(&self) -> Option<String> {
        self.simulation_freshness().stale_reason(&self.session)
    }

    /// Is a GUI job in flight?
    ///
    /// The policy question (§28 ruling 8): one GUI job at a time. The
    /// submit site asks it and refuses with a toast. It is NOT a drawing
    /// question — WP24 deleted the full-screen placeholder that made it
    /// one.
    #[must_use]
    pub fn is_job_running(&self) -> bool {
        self.job_run.is_some()
    }

    /// Modal exclusivity (density pass Batch 2): every modal-open path
    /// calls this first, so opening one modal closes the others — the
    /// 2026-06-11 capture sweep produced a 3-deep stack (a spinner over
    /// Tool Library over a feeds modal). A planner whose preview is in
    /// flight survives: closing it here would discard the user's run.
    pub fn close_modals_for_exclusivity(&mut self) {
        self.feeds_modal = None;
        self.tool_library_modal = None;
        self.machine_library_open = false;
        self.preferences = None;
        self.show_export_wizard = false;
        self.show_preflight = false;
        self.show_shortcuts = false;
        self.simulation.open_trace = None;
        if !self.is_job_running() {
            // Closed, not discarded — see the field doc. A planner whose
            // preview is in flight is spared: closing it would throw away
            // a walk the operator is waiting on.
            if let Some(planner) = self.multitool_planner.as_mut() {
                planner.open = false;
            }
        }
    }

    /// Open the trace modal on `metric`. Modal exclusivity: the other
    /// modals close first.
    pub fn open_cut_metric_trace(&mut self, metric: rs_cam_core::tool_load::DistributionMetric) {
        self.close_modals_for_exclusivity();
        self.simulation.open_trace = Some(metric);
        self.simulation.trace_window = None;
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feeds_modal_fixture() -> FeedsModalState {
        FeedsModalState {
            toolpath_id: rs_cam_core::ToolpathId::from_raw(0),
            explore: None,
        }
    }

    /// Density pass Batch 2 — opening any modal closes the others. The
    /// 2026-06-11 capture sweep stacked a spinner over Tool Library over
    /// a feeds modal; exclusivity makes that state unrepresentable.
    #[test]
    fn close_modals_for_exclusivity_closes_settled_modals() {
        let mut state = AppState::new();
        state.feeds_modal = Some(feeds_modal_fixture());
        state.tool_library_modal = Some(ToolLibraryModalState { catalogs: vec![] });
        state.show_export_wizard = true;
        state.show_preflight = true;
        state.preferences = Some(crate::ui::preferences::PreferencesState::new(
            &rs_cam_core::settings::LoadedSettings::default(),
            rs_cam_core::budget::MemoryBudget::UNLIMITED,
            None,
        ));

        state.close_modals_for_exclusivity();

        assert!(state.feeds_modal.is_none());
        assert!(state.tool_library_modal.is_none());
        assert!(!state.show_export_wizard);
        assert!(!state.show_preflight);
        assert!(state.preferences.is_none(), "the Preferences window closes");
    }
}
