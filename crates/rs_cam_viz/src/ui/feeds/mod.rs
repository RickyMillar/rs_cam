//! The feeds and speeds domain: the Feeds tab's chart and recommendation
//! card (W3i), the vendor data window, and the helpers they read.
//!
//! The directory is named after the domain, not after one surface. W3i
//! (2026-10-30) deleted the Explore window: the chart and the card are
//! drawn inline on the Feeds tab of the inspector.
//!
//! # The map
//!
//! | File | Holds | Who draws it |
//! |---|---|---|
//! | [`vendor`] | the vendor cutting data window | `app.rs`, through [`draw`] |
//! | [`explore`] | the nomogram and the inline block (`draw_inline`) | the Feeds tab |
//! | [`compare`] | the recommendation: Apply, problems, one `Details` | [`explore`] (W3i) |
//! | [`why`] | the per-row sentences behind the recommendation | [`compare`] |
//! | [`shared`] | what more than one surface needs | the inspector and `ui/readiness_panel.rs` |
//!
//! [`shared`] has three consumers, the third being the project rollup in
//! `ui/readiness_panel.rs`. That is why the inspector half stays here: moving
//! it to `ui/properties/feeds/` would either duplicate [`shared`] or leave a
//! worse cross-directory dependency.
//!
//! # How the split came about (DC5a)
//!
//! DC5a measured `ui/feeds_modal.rs` at 3 450 lines and found that its draw
//! functions divide into four jobs that share nothing but a window, at TWO
//! different scopes:
//!
//! | File | Job | Scope |
//! |---|---|---|
//! | [`compare`] | compare and apply | this operation |
//! | [`why`] | the detail behind the recommendation | this operation |
//! | [`explore`] | the nomogram and the mini charts | this operation |
//! | [`shared`] | what more than one job needs | — |
//!
//! There was a FOURTH job, and it is the finding: a rollup over every
//! toolpath, at THE WHOLE PROJECT's scope, held in this window behind a
//! `FeedsModalMode` flip. A container holds one scope, so the rollup left.
//! It lives in `ui/readiness_panel.rs`, on the workspace that already
//! answers project-wide questions, and it took its own state with it
//! (`AppState::project_feeds`). The mode flip is deleted.
//!
//! That split moved every function VERBATIM. Only the visibility and the
//! `use` paths changed, so the split and the behaviour change can be reviewed
//! apart. W3i then deleted the window that held the one remaining scope:
//! its chart and card are on the Feeds tab.

pub(crate) mod compare;
pub(crate) mod explore;
pub(crate) mod shared;
pub(crate) mod vendor;
pub(crate) mod why;

/// `app.rs` draws the vendor data window through this path.
pub use vendor::draw;
