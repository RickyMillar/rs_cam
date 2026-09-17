//! CRUD mutation methods on [`ProjectSession`].
//!
//! Every mutation here reports [`Effects`] — the toolpath indices whose
//! generation inputs moved, whether it cleared the simulation, and the
//! revision of the one toolpath it names. It builds none of that itself:
//! it runs its body inside
//! [`ProjectSession::try_with_effects`](ProjectSession::try_with_effects),
//! the one construction site (`session/command.rs`).
//!
//! P4 split the body into three children, one per entity group:
//! `mutation/toolpath.rs`, `mutation/entities.rs` and
//! `mutation/config.rs`. Each holds inherent methods on
//! [`ProjectSession`], so no path moved and no re-export is needed.

use crate::geo::{BoundingBox3, P3};
use crate::polygon::Polygon2;

use super::{Fixture, KeepOutZone};

// Path anchors for the children. A child names each of these as
// `super::Name`, in code or in a doc link, so the binding must sit in
// this module.
// They stay PRIVATE. A private item is visible to the module that holds it
// and to every descendant, so a child names it through `super::`. A
// `pub(super)` binding would publish these names into `session` as well,
// which the split does not need.
use super::{DatumConfig, LoadedModel, ToolpathComputeResult};
// `Command` and `RestoreToolpathSnapshotArgs` carry no code reference. Only
// a doc link in `mutation/toolpath.rs` names them, and rustc does not count
// a doc link as a use.
#[allow(unused_imports)]
use super::{Command, RestoreToolpathSnapshotArgs};

mod config;
mod entities;
mod toolpath;

/// Whether a fixture edit moved something a collision check reads.
///
/// Every field except the NAME is an input of the holder-clearance and
/// fixture-collision checks. The name reaches the setup sheet alone, so
/// a rename must not drop a result (WP6).
///
/// It compares the whole record with the name equalised, so a field
/// ADDED to `Fixture` later counts as a collision input until someone
/// states otherwise. That is the safe default.
fn fixture_collision_inputs_moved(before: &Fixture, after: &Fixture) -> bool {
    let mut named_before = before.clone();
    named_before.name.clone_from(&after.name);
    named_before != *after
}

/// Whether a keep-out edit moved something a collision check reads.
///
/// The twin of [`fixture_collision_inputs_moved`], and it exempts the
/// same one field.
fn keep_out_collision_inputs_moved(before: &KeepOutZone, after: &KeepOutZone) -> bool {
    let mut named_before = before.clone();
    named_before.name.clone_from(&after.name);
    named_before != *after
}

/// Whether a post-config edit moved a field that reaches emitted motion
/// or the simulated clock (WP17).
///
/// `true` names the fields whose old value is baked into a cached
/// artefact — the stored toolpaths, or the cut trace. The two fields it
/// exempts are read LIVE, so no cached artefact can hold a stale copy of
/// either:
///
/// - `format`, the post flavour, which
///   [`crate::gcode::export_gcode_checked`] resolves from
///   [`ProjectSession::post_config`] at emit time;
/// - `spindle_strategy`, which the feeds suggest path reads at suggest
///   time (`ProjectSession::feeds_result_for_toolpath`).
///
/// It follows [`fixture_collision_inputs_moved`]: it equalises the two
/// exempt fields and compares the WHOLE record, so a field ADDED to
/// [`crate::gcode::PostConfig`] later counts as a motion input until someone
/// states otherwise. That is the safe default.
fn post_change_reaches_motion(
    before: &crate::gcode::PostConfig,
    after: &crate::gcode::PostConfig,
) -> bool {
    let mut exempt_before = before.clone();
    exempt_before.format.clone_from(&after.format);
    exempt_before.spindle_strategy = after.spindle_strategy;
    exempt_before != *after
}

/// Compute a 3D bounding box from a slice of 2D polygons (SVG/DXF models).
/// Z extent is zero; `update_from_bbox` preserves stock Z for 2D models.
/// Returns `None` if all polygons are empty.
///
/// C22(a): `pub` because the GUI asks the same question. C22(a) deleted
/// the GUI's own copy in `state/job.rs`, which was written the other way
/// round (`min_x.min(pt.x)` against `if pt.x < min_x`) but gave the same
/// answer. The copy existed only because this one was `pub(crate)`.
pub fn polygons_bbox(polygons: &[Polygon2]) -> Option<BoundingBox3> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for poly in polygons {
        for pt in poly
            .exterior
            .iter()
            .chain(poly.holes.iter().flat_map(|h| h.iter()))
        {
            if pt.x < min_x {
                min_x = pt.x;
            }
            if pt.y < min_y {
                min_y = pt.y;
            }
            if pt.x > max_x {
                max_x = pt.x;
            }
            if pt.y > max_y {
                max_y = pt.y;
            }
        }
    }
    if !min_x.is_finite() {
        return None;
    }
    Some(BoundingBox3 {
        min: P3::new(min_x, min_y, 0.0),
        max: P3::new(max_x, max_y, 0.0),
    })
}

#[cfg(test)]
mod tests;
