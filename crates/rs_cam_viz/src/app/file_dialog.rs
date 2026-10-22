//! F218: the one door to a native file dialog.
//!
//! A blocking `rfd` dialog used to run inside the frame. While it was open
//! the frame stopped, and so did the MCP drain that runs once per frame.
//! Every dialog now goes through this file:
//!
//! 1. A draw (or an app handler) builds a [`FileDialogRequest`] and names
//!    what the pick is for with an [`AfterPick`].
//! 2. [`FileDialogSlot::open`] runs the dialog on its own thread. One
//!    dialog is open at a time; a second request while one is open does
//!    nothing.
//! 3. The thread sends the path back and asks for a repaint.
//!    [`FileDialogSlot::poll`] gives it to `handle_events`, which posts
//!    `AppEvent::FileDialog(FileDialogEvent::Picked { .. })`.
//! 4. [`RsCamApp::handle_file_dialog_event`] runs the continuation through
//!    the same calls the old in-frame code made.
//!
//! The sentry `file_dialogs_run_off_the_frame_f218` refuses an
//! `rfd::FileDialog::new()` anywhere else in `src/`.
//!
//! rfd 0.15 on Linux uses the XDG desktop portal (D-Bus) with a zenity
//! fallback, so a dialog on a worker thread is safe there. On macOS rfd
//! sends a dialog from another thread to the main thread itself.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::state::job::ModelId;
use crate::ui::AppEvent;

use super::RsCamApp;

/// Which native dialog to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogKind {
    PickFile,
    SaveFile,
    PickFolder,
}

/// A file dialog as plain data. The thread builds the `rfd` dialog from
/// it, so the `rfd` type never crosses a thread and this file holds the
/// only construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDialogRequest {
    kind: DialogKind,
    filters: Vec<(String, Vec<String>)>,
    file_name: Option<String>,
    directory: Option<PathBuf>,
}

impl FileDialogRequest {
    fn new(kind: DialogKind) -> Self {
        Self {
            kind,
            filters: Vec::new(),
            file_name: None,
            directory: None,
        }
    }

    /// An "open one file" dialog.
    pub(crate) fn pick_file() -> Self {
        Self::new(DialogKind::PickFile)
    }

    /// A "save as" dialog.
    pub(crate) fn save_file() -> Self {
        Self::new(DialogKind::SaveFile)
    }

    /// A "choose a folder" dialog.
    pub(crate) fn pick_folder() -> Self {
        Self::new(DialogKind::PickFolder)
    }

    /// Add one named filter (`rfd::FileDialog::add_filter`).
    pub(crate) fn filter(mut self, name: &str, extensions: &[&str]) -> Self {
        self.filters.push((
            name.to_owned(),
            extensions.iter().map(|e| (*e).to_owned()).collect(),
        ));
        self
    }

    /// The suggested file name of a save dialog.
    pub(crate) fn file_name(mut self, name: &str) -> Self {
        self.file_name = Some(name.to_owned());
        self
    }

    /// The start folder, when there is one.
    pub(crate) fn directory(mut self, directory: Option<&Path>) -> Self {
        self.directory = directory.map(Path::to_path_buf);
        self
    }

    /// The event a draw pushes to open this dialog for `then`.
    pub(crate) fn event(self, then: AfterPick) -> AppEvent {
        AppEvent::FileDialog(FileDialogEvent::Open {
            request: self,
            then,
        })
    }

    /// Show the dialog and wait for the answer. Runs on the dialog thread
    /// only ([`FileDialogSlot::open`]).
    fn run(self) -> Option<PathBuf> {
        let Self {
            kind,
            filters,
            file_name,
            directory,
        } = self;
        let mut dialog = rfd::FileDialog::new();
        for (name, extensions) in &filters {
            dialog = dialog.add_filter(name, extensions);
        }
        if let Some(name) = file_name {
            dialog = dialog.set_file_name(name);
        }
        if let Some(directory) = directory {
            dialog = dialog.set_directory(directory);
        }
        match kind {
            DialogKind::PickFile => dialog.pick_file(),
            DialogKind::SaveFile => dialog.save_file(),
            DialogKind::PickFolder => dialog.pick_folder(),
        }
    }
}

/// File contents that wait for a save path. `Debug` prints the length
/// only, so an event log does not print a whole G-code file.
#[derive(Clone, PartialEq, Eq)]
pub struct Contents(pub(crate) String);

impl std::fmt::Debug for Contents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Contents({} bytes)", self.0.len())
    }
}

/// The folder layouts of the export wizard. They differ in the
/// notification only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderLayout {
    PerSetup,
    PerToolpath,
}

/// What the app does with a picked path. Each arm is the code that ran
/// after the old blocking dialog returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AfterPick {
    /// Import the model through `model_import_event` (File menu, Setup rail).
    ImportModel,
    /// Point a model at a different file (`AppEvent::RelinkModel`).
    RelinkModel(ModelId),
    /// Read a GRBL `$$` dump into the Machine panel's buffer.
    LoadGrblSettings,
    /// Open a project file and fit the camera.
    OpenJob,
    /// Save the project (File ▸ Save Job on a project with no path).
    SaveJob,
    /// Save the project for the unsaved-changes dialog, then run what it
    /// guards. The guard is `RsCamApp::unsaved_guard` at the pick: the
    /// dialog's answers are disabled while the save dialog is open.
    SaveJobForUnsavedGuard,
    /// Write G-code with the `write_one` notifications (the direct export).
    WriteGcode(Contents),
    /// The export wizard's single file: write, remember the folder, close.
    WizardFile(Contents),
    /// The export wizard's per-setup or per-toolpath files. Each pair is
    /// (file name, contents); the names are rendered before the dialog.
    WizardFolder {
        layout: FolderLayout,
        files: Vec<(String, Contents)>,
    },
    /// File ▸ Export combined G-code.
    WriteCombinedGcode(Contents),
    /// Export the G-code of one setup.
    WriteSetupGcode { setup_name: String, gcode: Contents },
    /// Write the setup sheet HTML.
    WriteSetupSheet(Contents),
    /// Write the SVG preview.
    WriteSvgPreview(Contents),
}

/// The one `AppEvent` variant of a file dialog.
#[derive(Debug)]
pub enum FileDialogEvent {
    /// Open this dialog (refused while one is open).
    Open {
        request: FileDialogRequest,
        then: AfterPick,
    },
    /// The operator picked `path`; run `then`.
    Picked { path: PathBuf, then: AfterPick },
}

/// The dialog that is open now, and what its pick is for.
struct Pending {
    then: AfterPick,
    answer: mpsc::Receiver<Option<PathBuf>>,
}

/// The one open file dialog of the app. `None` inside: no dialog is open.
pub(crate) struct FileDialogSlot {
    ctx: egui::Context,
    pending: Option<Pending>,
}

impl FileDialogSlot {
    pub(crate) fn new(ctx: egui::Context) -> Self {
        Self { ctx, pending: None }
    }

    /// Is a dialog open now?
    pub(crate) fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    /// Show `request` on a dialog thread. Returns `false` and does nothing
    /// when a dialog is open already, or when the thread cannot start.
    pub(crate) fn open(&mut self, request: FileDialogRequest, then: AfterPick) -> bool {
        if self.pending.is_some() {
            return false;
        }
        let (send, answer) = mpsc::channel();
        let ctx = self.ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("file-dialog".to_owned())
            .spawn(move || {
                let picked = request.run();
                // The app may have closed; a lost answer is harmless.
                let _ = send.send(picked);
                // A parked frame would otherwise wait for the next input.
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => {
                self.pending = Some(Pending { then, answer });
                true
            }
            Err(error) => {
                tracing::error!("Failed to start the file dialog thread: {error}");
                false
            }
        }
    }

    /// The pick of the open dialog, once it has closed. A cancel or a lost
    /// thread frees the slot and returns `None`.
    pub(crate) fn poll(&mut self) -> Option<(PathBuf, AfterPick)> {
        let pending = self.pending.as_ref()?;
        let answer = match pending.answer.try_recv() {
            Ok(answer) => answer,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                tracing::error!("The file dialog thread ended without an answer");
                None
            }
        };
        let Pending { then, .. } = self.pending.take()?;
        answer.map(|path| (path, then))
    }

    /// A slot with an open dialog whose answer the test sends.
    #[cfg(test)]
    fn with_pending(then: AfterPick) -> (Self, mpsc::Sender<Option<PathBuf>>) {
        let (send, answer) = mpsc::channel();
        let slot = Self {
            ctx: egui::Context::default(),
            pending: Some(Pending { then, answer }),
        };
        (slot, send)
    }
}

impl RsCamApp {
    /// Post the pick of a closed dialog as an `AppEvent`, for this frame's
    /// `handle_events`.
    pub(super) fn post_file_dialog_pick(&mut self) {
        if let Some((path, then)) = self.file_dialogs.poll() {
            self.controller
                .events_mut()
                .push(AppEvent::FileDialog(FileDialogEvent::Picked { path, then }));
        }
    }

    /// Open a dialog from app code. `false`: a dialog is open already.
    pub(super) fn open_file_dialog(&mut self, request: FileDialogRequest, then: AfterPick) -> bool {
        self.file_dialogs.open(request, then)
    }

    /// Run one `AppEvent::FileDialog`.
    pub(super) fn handle_file_dialog_event(&mut self, ctx: &egui::Context, event: FileDialogEvent) {
        match event {
            FileDialogEvent::Open { request, then } => {
                let _ = self.open_file_dialog(request, then);
            }
            FileDialogEvent::Picked { path, then } => self.after_pick(ctx, path, then),
        }
    }

    /// The continuation of a pick: the code that ran after the old
    /// blocking dialog returned `Some(path)`.
    fn after_pick(&mut self, ctx: &egui::Context, path: PathBuf, then: AfterPick) {
        match then {
            AfterPick::ImportModel => {
                if let Some(event) = crate::ui::model_import_event(path) {
                    self.post_next_frame(ctx, event);
                }
            }
            AfterPick::RelinkModel(id) => {
                self.post_next_frame(ctx, AppEvent::RelinkModel(id, path));
            }
            AfterPick::LoadGrblSettings => {
                let grbl = &mut self.controller.state_mut().panels.grbl;
                match std::fs::read_to_string(&path) {
                    Ok(content) => grbl.buffer = content,
                    Err(e) => {
                        tracing::error!("read $$ file failed: {e}");
                        grbl.status = format!("Read failed: {e}");
                    }
                }
            }
            AfterPick::OpenJob => self.open_job_from_picked(&path),
            AfterPick::SaveJob => self.save_job_to_picked(&path),
            AfterPick::SaveJobForUnsavedGuard => self.save_for_guard_to_picked(ctx, &path),
            AfterPick::WriteGcode(gcode) => self.write_one(&path, &gcode.0),
            AfterPick::WizardFile(gcode) => {
                self.write_one(&path, &gcode.0);
                self.remember_dir(path.parent());
                self.close_wizard();
            }
            AfterPick::WizardFolder { layout, files } => {
                self.write_wizard_folder(&path, layout, &files);
            }
            AfterPick::WriteCombinedGcode(gcode) => {
                if let Err(error) = std::fs::write(&path, &gcode.0) {
                    self.controller.push_notification(
                        format!("Failed to write G-code: {error}"),
                        crate::controller::Severity::Error,
                    );
                } else {
                    tracing::info!("Exported combined G-code to {}", path.display());
                    self.controller.push_notification(
                        format!("Exported combined G-code to {}", path.display()),
                        crate::controller::Severity::Info,
                    );
                }
            }
            AfterPick::WriteSetupGcode { setup_name, gcode } => {
                if let Err(error) = std::fs::write(&path, &gcode.0) {
                    self.controller.push_notification(
                        format!("Failed to write G-code: {error}"),
                        crate::controller::Severity::Error,
                    );
                } else {
                    tracing::info!(
                        "Exported setup '{}' G-code to {}",
                        setup_name,
                        path.display()
                    );
                    self.controller.push_notification(
                        format!(
                            "Exported setup '{}' G-code to {}",
                            setup_name,
                            path.display()
                        ),
                        crate::controller::Severity::Info,
                    );
                }
            }
            AfterPick::WriteSetupSheet(html) => {
                if let Err(error) = std::fs::write(&path, &html.0) {
                    tracing::error!("Failed to write setup sheet: {error}");
                    self.controller.push_notification(
                        format!("Failed to write setup sheet: {error}"),
                        crate::controller::Severity::Error,
                    );
                } else {
                    tracing::info!("Exported setup sheet to {}", path.display());
                    self.controller.push_notification(
                        format!("Exported setup sheet to {}", path.display()),
                        crate::controller::Severity::Info,
                    );
                }
            }
            AfterPick::WriteSvgPreview(svg) => {
                if let Err(error) = std::fs::write(&path, &svg.0) {
                    tracing::error!("Failed to write SVG: {error}");
                    self.controller.push_notification(
                        format!("Failed to write SVG: {error}"),
                        crate::controller::Severity::Error,
                    );
                } else {
                    tracing::info!("Exported SVG preview to {}", path.display());
                    self.controller.push_notification(
                        format!("Exported SVG preview to {}", path.display()),
                        crate::controller::Severity::Info,
                    );
                }
            }
        }
    }

    /// Queue `event` for the next frame and ask for that frame. This
    /// frame's events were drained before the pick ran.
    fn post_next_frame(&mut self, ctx: &egui::Context, event: AppEvent) {
        self.controller.events_mut().push(event);
        ctx.request_repaint();
    }

    /// File ▸ Save Job on a project with no path, after the pick.
    pub(super) fn save_job_to_picked(&mut self, path: &Path) {
        match self.controller.save_job_to_path(path) {
            Ok(()) => {
                tracing::info!("Saved job to {}", path.display());
                self.controller.push_notification(
                    format!("Saved job to {}", path.display()),
                    crate::controller::Severity::Info,
                );
            }
            Err(error) => self.controller.push_error(&error),
        }
    }

    /// Save & <verb> of the unsaved-changes dialog, after the pick. A
    /// failed save keeps the dialog open. The guarded action runs only
    /// when the dialog still guards one.
    fn save_for_guard_to_picked(&mut self, ctx: &egui::Context, path: &Path) {
        match self.controller.save_job_to_path(path) {
            Ok(()) => {
                tracing::info!("Saved job to {}", path.display());
                if let Some(guard) = self.unsaved_guard.take() {
                    self.proceed_past_unsaved_guard(ctx, guard);
                }
            }
            Err(error) => self.controller.push_error(&error),
        }
    }

    /// The per-setup and per-toolpath layouts of the export wizard, after
    /// the folder pick.
    fn write_wizard_folder(
        &mut self,
        dir: &Path,
        layout: FolderLayout,
        files: &[(String, Contents)],
    ) {
        let mut written = 0usize;
        for (name, gcode) in files {
            let path = dir.join(name);
            if let Err(e) = std::fs::write(&path, &gcode.0) {
                self.controller.push_notification(
                    format!("Failed to write {}: {e}", path.display()),
                    crate::controller::Severity::Error,
                );
                return;
            }
            written += 1;
        }
        self.remember_dir(Some(dir));
        let message = match layout {
            FolderLayout::PerSetup => {
                format!("Exported {written} setup file(s) to {}", dir.display())
            }
            FolderLayout::PerToolpath => {
                format!("Exported {written} toolpath file(s) to {}", dir.display())
            }
        };
        self.controller
            .push_notification(message, crate::controller::Severity::Info);
        self.close_wizard();
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    #[test]
    fn a_second_request_while_one_is_open_does_nothing_f218() {
        let (mut slot, _send) = FileDialogSlot::with_pending(AfterPick::OpenJob);
        assert!(slot.is_open());
        // The refusal returns before a thread starts, so no dialog shows.
        assert!(!slot.open(FileDialogRequest::pick_file(), AfterPick::SaveJob));
        assert!(slot.is_open(), "the first dialog stays the open one");
    }

    #[test]
    fn the_pick_comes_back_once_with_its_continuation_f218() {
        let (mut slot, send) = FileDialogSlot::with_pending(AfterPick::LoadGrblSettings);
        assert!(slot.poll().is_none(), "no answer yet: nothing to post");
        assert!(slot.is_open());
        send.send(Some(PathBuf::from("/tmp/grbl.txt"))).unwrap();
        let (path, then) = slot.poll().expect("the answer is posted");
        assert_eq!(path, PathBuf::from("/tmp/grbl.txt"));
        assert_eq!(then, AfterPick::LoadGrblSettings);
        assert!(!slot.is_open(), "the slot is free after the pick");
        assert!(slot.poll().is_none(), "the pick is posted once");
    }

    #[test]
    fn a_cancel_or_a_lost_thread_frees_the_slot_and_posts_nothing_f218() {
        let (mut slot, send) = FileDialogSlot::with_pending(AfterPick::SaveJob);
        send.send(None).unwrap();
        assert!(slot.poll().is_none());
        assert!(!slot.is_open());

        let (mut slot, send) = FileDialogSlot::with_pending(AfterPick::SaveJob);
        drop(send);
        assert!(slot.poll().is_none());
        assert!(!slot.is_open());
    }

    #[test]
    fn the_request_keeps_every_dialog_option_f218() {
        let request = FileDialogRequest::save_file()
            .filter("G-code", &["nc", "gcode", "ngc"])
            .file_name("output.nc")
            .directory(Some(Path::new("/tmp")));
        assert_eq!(request.kind, DialogKind::SaveFile);
        assert_eq!(
            request.filters,
            vec![(
                "G-code".to_owned(),
                vec!["nc".to_owned(), "gcode".to_owned(), "ngc".to_owned()]
            )]
        );
        assert_eq!(request.file_name.as_deref(), Some("output.nc"));
        assert_eq!(request.directory.as_deref(), Some(Path::new("/tmp")));
        assert_eq!(
            FileDialogRequest::pick_folder().directory(None).directory,
            None
        );
    }

    #[test]
    fn contents_debug_prints_the_length_only_f218() {
        let text = format!("{:?}", Contents("G0 X0\n".to_owned()));
        assert_eq!(text, "Contents(6 bytes)");
    }
}
