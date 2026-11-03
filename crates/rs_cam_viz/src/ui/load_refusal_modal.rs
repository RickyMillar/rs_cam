//! G-D (§4): the "Project not opened" modal. An interactive open that the
//! loader refuses shows here, not as an 8 s toast.
//!
//! The frame is the load-warnings modal's (`app.rs`): `egui::Modal`, a
//! `NoticeStack` in a scroll area, then the buttons. One row per retired
//! key, its fix on the row hover, one fix line under the list when every
//! row shares it, and `Copy` puts the whole refusal on the clipboard.

use rs_cam_core::session::SessionError;

use crate::controller::LoadRefusal;
use crate::ui::components::{Notice, NoticeStack, Role, text};
use crate::ui::tokens;

/// The modal title.
pub const TITLE: &str = "Project not opened";

/// The fix line when the rows name different fixes.
pub const EACH_ROW_FIX: &str = "Each line's hover names its fix.";

const MODAL_WIDTH: f32 = 420.0;
const MODAL_MAX_LIST_HEIGHT: f32 = 280.0;

/// What the modal shows for one refusal, before any drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusalView {
    /// The file name; the full path is its hover.
    pub file_name: String,
    pub path: String,
    /// One row per retired key, or one row with the error text.
    pub notices: Vec<Notice>,
    /// The fix line under the list. `None` for an error with no fix.
    pub fix: Option<String>,
}

impl RefusalView {
    /// The view of `refusal`. A `RetiredKeys` error lists each hit; any
    /// other load error is one row with its own text.
    #[must_use]
    pub fn of(refusal: &LoadRefusal) -> Self {
        let path = refusal.path.display().to_string();
        let file_name = refusal
            .path
            .file_name()
            .map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned());
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "pass-through: every other variant keeps its own value (named: SessionError::RetiredKeys)"
        )]
        let (notices, fix) = match refusal.error.as_ref() {
            SessionError::RetiredKeys(found) => {
                let notices = found
                    .iter()
                    .map(|hit| Notice::new(Role::Danger, hit.path.clone()).detail(hit.replacement))
                    .collect();
                let fix = match found.first() {
                    Some(first) if found.iter().all(|hit| hit.replacement == first.replacement) => {
                        first.replacement.to_owned()
                    }
                    _ => EACH_ROW_FIX.to_owned(),
                };
                (notices, Some(fix))
            }
            other => (vec![Notice::new(Role::Danger, other.to_string())], None),
        };
        Self {
            file_name,
            path,
            notices,
            fix,
        }
    }

    /// The clipboard text of `Copy`: the path, each row with its fix, and
    /// the fix line, one per line.
    #[must_use]
    pub fn copy_text(&self) -> String {
        let mut lines = vec![self.path.clone()];
        for notice in &self.notices {
            match notice.detail.as_deref() {
                Some(detail) => lines.push(format!("{}  ({detail})", notice.text)),
                None => lines.push(notice.text.clone()),
            }
        }
        if let Some(fix) = &self.fix {
            lines.push(fix.clone());
        }
        lines.join("\n")
    }
}

/// Draw the modal for `refusal`. True when the operator closed it.
pub fn draw(ctx: &egui::Context, refusal: &LoadRefusal) -> bool {
    let view = RefusalView::of(refusal);
    let expand_id = egui::Id::new("load_refusal_expanded");
    let expanded = ctx.data(|d| d.get_temp::<bool>(expand_id).unwrap_or(false));
    let modal = egui::Modal::new(egui::Id::new("load_refusal_modal"))
        .backdrop_color(tokens::SCRIM)
        .show(ctx, |ui| {
            ui.set_max_width(MODAL_WIDTH);
            let response = ui.label(text::display(TITLE));
            crate::ui::automation::record(ui, "project_load_refusal", &response, TITLE);
            ui.label(text::caption(&view.file_name))
                .on_hover_text(&view.path);
            ui.add_space(tokens::SPACE_3);
            // Rule F: the container scrolls rather than clips.
            let expand_clicked = egui::ScrollArea::vertical()
                .max_height(MODAL_MAX_LIST_HEIGHT)
                .show(ui, |ui| {
                    NoticeStack::new(view.notices.clone())
                        .expanded(expanded)
                        .show(ui)
                })
                .inner;
            if let Some(fix) = &view.fix {
                ui.add_space(tokens::SPACE_2);
                ui.label(egui::RichText::new(fix).color(tokens::TEXT_BODY));
            }
            ui.add_space(tokens::SPACE_3);
            let (copy, close) = ui
                .horizontal(|ui| {
                    let close = ui
                        .add(crate::ui::components::Button::primary("Close"))
                        .clicked();
                    let copy = ui.add(crate::ui::components::Button::new("Copy")).clicked();
                    (copy, close)
                })
                .inner;
            (expand_clicked, copy, close)
        });
    let (expand_clicked, copy_clicked, close_clicked) = modal.inner;
    if expand_clicked {
        ctx.data_mut(|d| d.insert_temp(expand_id, true));
    }
    if copy_clicked {
        ctx.copy_text(view.copy_text());
    }
    let closed = close_clicked || modal.should_close();
    if closed {
        ctx.data_mut(|d| d.insert_temp(expand_id, false));
    }
    closed
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use rs_cam_core::session::RetiredKeyFound;

    fn refusal(error: SessionError) -> LoadRefusal {
        LoadRefusal {
            path: std::path::PathBuf::from("/jobs/wanaka_back.toml"),
            error: Box::new(error),
        }
    }

    const FIX: &str = "Set clearing_strategy to geodesic_adaptive or contour_parallel";

    /// Two retired values: two danger rows with the fix on hover, one fix
    /// line, and `Copy` holds the path, each row and the fix.
    #[test]
    fn a_retired_value_refusal_lists_each_hit_g_d() {
        let view = RefusalView::of(&refusal(SessionError::RetiredKeys(vec![
            RetiredKeyFound {
                path:
                    "setups[0].toolpaths[2].operation.params.clearing_strategy = \"agent_search\""
                        .to_owned(),
                replacement: FIX,
            },
            RetiredKeyFound {
                path:
                    "setups[0].toolpaths[4].operation.params.clearing_strategy = \"contour_spiral\""
                        .to_owned(),
                replacement: FIX,
            },
        ])));
        assert_eq!(view.file_name, "wanaka_back.toml");
        assert_eq!(view.notices.len(), 2);
        assert!(view.notices.iter().all(|n| n.role == Role::Danger));
        assert_eq!(view.notices[1].detail.as_deref(), Some(FIX));
        assert_eq!(view.fix.as_deref(), Some(FIX));
        let copy = view.copy_text();
        assert_eq!(copy.lines().count(), 4, "path, two hits, the fix: {copy}");
        assert!(copy.starts_with("/jobs/wanaka_back.toml\n"));
        assert!(copy.contains("\"contour_spiral\""));
    }

    /// Two different fixes: the fix line points to the row hovers.
    #[test]
    fn mixed_fixes_point_to_the_hovers_g_d() {
        let view = RefusalView::of(&refusal(SessionError::RetiredKeys(vec![
            RetiredKeyFound {
                path: "setups[0].fixtures".to_owned(),
                replacement: "Removed",
            },
            RetiredKeyFound {
                path: "setups[0].toolpaths[0].operation.params.clearing_strategy = \"adaptive\""
                    .to_owned(),
                replacement: FIX,
            },
        ])));
        assert_eq!(view.fix.as_deref(), Some(EACH_ROW_FIX));
    }

    /// Any other load error: the same modal, one row with its text, no fix.
    #[test]
    fn another_load_error_is_one_row_g_d() {
        let error = SessionError::MissingOperation {
            setup: "Setup 1".to_owned(),
            toolpath: "Edges".to_owned(),
        };
        let text = error.to_string();
        let view = RefusalView::of(&refusal(error));
        assert_eq!(view.notices.len(), 1);
        assert_eq!(view.notices[0].text, text);
        assert_eq!(view.fix, None);
    }

    /// The modal paints the title, the file, both hits and the fix, and
    /// no toast: the refusal is not a notification.
    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
    )]
    fn the_modal_paints_the_refusal_g_d() {
        let refusal = refusal(SessionError::RetiredKeys(vec![RetiredKeyFound {
            path: "setups[0].toolpaths[2].operation.params.clearing_strategy = \"agent_search\""
                .to_owned(),
            replacement: FIX,
        }]));
        let ctx = egui::Context::default();
        tokens::apply(&ctx);
        tokens::apply_fonts(&ctx);
        let mut texts = Vec::new();
        for _ in 0..3 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let closed = draw(ui.ctx(), &refusal);
                assert!(!closed);
            });
            texts = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::epaint::Shape::Text(t) => Some(t.galley.job.text.clone()),
                    _ => None,
                })
                .collect();
            out.textures_delta.clear();
        }
        for want in [TITLE, "wanaka_back.toml", FIX, "Copy", "Close"] {
            assert!(
                texts.iter().any(|t| t == want),
                "`{want}` missing: {texts:?}"
            );
        }
        assert!(
            texts.iter().any(|t| t.contains("\"agent_search\"")),
            "the hit is a row: {texts:?}"
        );
    }
}
