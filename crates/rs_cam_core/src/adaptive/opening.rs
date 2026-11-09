//! The opening of an adaptive piece: the starter pocket (feeds contract
//! P22b, `P22_DESIGN.md` section 3 A). Moved out of `path.rs` with no
//! change; the 2D door and the geodesic plan call it.

use super::path::LinkLoad;
use super::search::{
    PassLoad, ToolCentreRegion, find_entry_via_distance_transform, step_within_pass_load,
};
use super::{AdaptiveRuntimeEvent, AdaptiveSegment, MaterialGrid};
use crate::geo::P2;
use crate::polygon::Polygon2;

/// The bore a helix entry cuts (feeds contract P22b, `P22_DESIGN.md`
/// section 3 A): the helix centre and the helix radius that fits there.
/// The hole radius is `R + helix_radius`.
///
/// One rule for the 2D planner hole (`LinkLoad::entry_hole`) and the 3D
/// geodesic entry: the helix radius shrinks to the largest whose circle
/// stays inside the tool-centre region (`dressup::contained_helix_radius`,
/// the function the entry dressup reads), and a fit under the core floor
/// of a tool that does not cut at its centre is no bore
/// (`dressup::helix_fit_holds`). Where no bore fits, the entry falls back
/// (ruling Q1: ramp, then plunge).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Bore {
    /// The helix centre: the entry point.
    pub(crate) centre: P2,
    /// The helix radius that fits at `centre` (mm).
    pub(crate) helix_radius: f64,
}

impl Bore {
    /// The bore at `centre` inside `region` (tool-centre pieces,
    /// [`bore_region`]) for the requested `helix_radius`, or `None` where
    /// no helix holds (the fit is 0, or under `core_floor_mm`).
    pub(crate) fn at(
        region: &[Polygon2],
        centre: P2,
        helix_radius: f64,
        core_floor_mm: f64,
    ) -> Option<Self> {
        let fit = Self::fit_mm(region, centre, helix_radius);
        crate::dressup::helix_fit_holds(fit, core_floor_mm).then_some(Self {
            centre,
            helix_radius: fit,
        })
    }

    /// The largest helix radius, at most `helix_radius`, whose circle about
    /// `centre` stays inside `region` (the fit a fallback finding names).
    pub(crate) fn fit_mm(region: &[Polygon2], centre: P2, helix_radius: f64) -> f64 {
        crate::dressup::contained_helix_radius(region, centre.x, centre.y, helix_radius)
    }

    /// The radius of the hole the bore leaves: `tool_radius + helix_radius`.
    pub(crate) fn hole_radius(&self, tool_radius: f64) -> f64 {
        tool_radius + self.helix_radius
    }
}

/// The tool-centre region of `outline` that a bore must stay inside: the
/// pieces the 2D planner holds every cutter centre to
/// (`ToolCentreRegion`, the region `LinkLoad::entry_hole` reads).
pub(crate) fn bore_region(outline: &Polygon2, tool_radius: f64) -> Vec<Polygon2> {
    ToolCentreRegion::new(outline, tool_radius)
        .pieces()
        .to_vec()
}

/// Emit a circular starter pocket centred on the largest inscribed
/// disk inside `machinable`, returning the cutter end position so the
/// engagement-target spiral can continue from there with full swing
/// room from move 1.
///
/// The cutter walks a circle of radius `tool_radius` around the
/// medial-axis maximum, clearing a disc of radius `2 × tool_radius`.
/// In 3D production this would be a helical plunge; for the 2D
/// planner it's a single circular pass after a Rapid + Z-plunge.
///
/// Returns `None` when no DT-maximum cell with ≥ `2 × tool_radius`
/// clearance exists (uniformly-narrow regions — the narrow gate
/// would already have handled those).
///
/// Reference: Autodesk patent US7831332 (Fusion HSM transition
/// portion), Bieterman & Sandström (Boeing, ~2003), Ren & Bi (2014).
pub(super) fn emit_helical_starter_pocket(
    grid: &mut MaterialGrid,
    machinable_mask: &[bool],
    boundary_distances: &[f64],
    tool_radius: f64,
    pass_load: Option<(&PassLoad, f64, &LinkLoad)>,
) -> Option<(Vec<AdaptiveSegment>, P2)> {
    // B10: under the 2D pass load the starter stands on material: on a
    // rest level the medial point can lie where an earlier op cleared.
    let medial = find_entry_via_distance_transform(
        grid,
        machinable_mask,
        boundary_distances,
        tool_radius,
        pass_load.is_some(),
    )?;
    // Need the medial-axis disk to fit the helix (radius `tool_radius`)
    // plus the cutter (radius `tool_radius`) plus a small safety margin.
    let dt_at = grid.boundary_distance_at(boundary_distances, medial.x, medial.y);
    let required = 2.0 * tool_radius;
    if dt_at < required {
        return None;
    }

    if let Some((load, stepover, link_load)) = pass_load {
        let entry_radius = link_load.entry_hole(medial);
        return emit_spiral_starter_pocket(grid, medial, tool_radius, stepover, load, entry_radius);
    }

    let helix_r = tool_radius;
    let n_steps = 64;
    let mut segments: Vec<AdaptiveSegment> = Vec::new();

    // Clear the medial cell itself (the Z-plunge centre).
    grid.clear_circle(medial.x, medial.y, tool_radius);

    let start = P2::new(medial.x + helix_r, medial.y);
    segments.push(AdaptiveSegment::Rapid(start));
    grid.clear_circle(start.x, start.y, tool_radius);

    let mut path: Vec<P2> = vec![start];
    for i in 1..=n_steps {
        let theta = (i as f64 / n_steps as f64) * std::f64::consts::TAU;
        let x = medial.x + helix_r * theta.cos();
        let y = medial.y + helix_r * theta.sin();
        grid.clear_circle(x, y, tool_radius);
        path.push(P2::new(x, y));
    }
    let end = *path.last()?;
    segments.push(AdaptiveSegment::Cut(path));
    Some((segments, end))
}

/// Angular steps per turn of the starter pocket (the historical helix's).
const STARTER_STEPS_PER_TURN: usize = 64;

/// The cutter-centre path of the 2D starter pocket: from the plunge at
/// `medial`, an Archimedean spiral `r = pitch · θ / 2π` out to `r = R`, then
/// one full lap at `R`. It clears the same `2R` disc the historical circle
/// did, but each lap bites at most `pitch` sideways instead of a full slot.
fn starter_spiral_points(medial: P2, tool_radius: f64, pitch: f64) -> Vec<P2> {
    let dtheta = std::f64::consts::TAU / STARTER_STEPS_PER_TURN as f64;
    let theta_end = std::f64::consts::TAU * tool_radius / pitch;
    let at = |r: f64, theta: f64| P2::new(medial.x + r * theta.cos(), medial.y + r * theta.sin());
    let mut pts = vec![medial];
    let mut i = 1usize;
    loop {
        let theta = i as f64 * dtheta;
        if theta >= theta_end {
            break;
        }
        pts.push(at(pitch * theta / std::f64::consts::TAU, theta));
        i += 1;
    }
    for k in 0..=STARTER_STEPS_PER_TURN {
        pts.push(at(tool_radius, theta_end + k as f64 * dtheta));
    }
    pts
}

/// G-ADAPTPASSLOAD: the 2D starter pocket. The historical circle of radius
/// R around the plunge ran a full slot (the sim read 0.83). Plunge at the
/// medial point (the disc the grid clears there), then spiral out
/// ([`starter_spiral_points`]) at pitch `min(stepover, R)`, halved up to
/// three times until every step holds [`step_within_pass_load`]. `None`
/// when no pitch does; the agent then enters from the boundary.
fn emit_spiral_starter_pocket(
    grid: &mut MaterialGrid,
    medial: P2,
    tool_radius: f64,
    stepover: f64,
    load: &PassLoad,
    entry_radius: f64,
) -> Option<(Vec<AdaptiveSegment>, P2)> {
    let mut pitch = stepover.min(tool_radius);
    for _ in 0..4 {
        let pts = starter_spiral_points(medial, tool_radius, pitch);
        let mut log = Vec::new();
        grid.clear_circle_logged(medial.x, medial.y, entry_radius.max(tool_radius), &mut log);
        let mut holds = true;
        for w in pts.windows(2) {
            let &[a, b] = w else {
                continue;
            };
            let angle = (b.y - a.y).atan2(b.x - a.x);
            // A straight plunge (no helix hole): the steps inside the
            // plunge hole are exempt (`PassLoad::departing`).
            let step_load = if entry_radius <= tool_radius
                && (b.x - medial.x).hypot(b.y - medial.y) < tool_radius
            {
                load.departing()
            } else {
                *load
            };
            if !step_within_pass_load(grid, a, b.x, b.y, tool_radius, angle, &step_load) {
                holds = false;
                break;
            }
            grid.clear_segment_logged(a, b, tool_radius, &mut log);
        }
        if holds {
            let end = *pts.last()?;
            let segments = vec![
                AdaptiveSegment::Marker(AdaptiveRuntimeEvent::StarterPocket {
                    center_x: medial.x,
                    center_y: medial.y,
                }),
                AdaptiveSegment::Rapid(medial),
                AdaptiveSegment::Cut(pts),
            ];
            return Some((segments, end));
        }
        grid.restore_cleared(&log);
        pitch *= 0.5;
    }
    None
}
