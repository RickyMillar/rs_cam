//! The feeds and speeds domain: the Explore window AND the inspector card.
//!
//! The directory is named after the domain, not after one surface. Half of
//! what is here is reached from the properties panel and never from the
//! window.
//!
//! # The map
//!
//! | File | Holds | Who draws it |
//! |---|---|---|
//! | [`window`] | the Explore window: frame, size cap, one body call | `app.rs`, through [`draw`] |
//! | [`explore`] | the nomogram and the window's body | [`window`] |
//! | [`compare`] | the comparison card: current vs recommended, and Apply | `ui/properties/mod.rs` |
//! | [`why`] | the per-row sentences behind the recommendation | `ui/properties/mod.rs` |
//! | [`shared`] | what more than one surface needs | [`window`], the inspector, and `ui/readiness_panel.rs` |
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
//! apart. D-3 then moved the window's own `draw` out of this file into
//! [`window`], again verbatim, so that this file is a map and nothing else.

pub(crate) mod compare;
pub(crate) mod explore;
pub(crate) mod shared;
pub(crate) mod why;
pub(crate) mod window;

/// `app.rs` draws the Explore window through this path. The window itself
/// lives in [`window`].
pub use window::draw;
