//! The obstacles in the viewport (K2): the obstacle itself as a line prism,
//! its clearance as a dim ring at the grown top, and the obstacle plane.
//!
//! An obstacle is in the setup frame and fixed to the table (core
//! `compute::obstacle`), so every vertex takes only the setup's
//! emission-to-display shift. The renderer resolves ONE obstacle at a time
//! (D10): the resolve refuses a whole set on one error, and one broken
//! obstacle must not blank the others. The list row shows the error.

use rs_cam_core::compute::obstacle::{ObstacleSet, ResolvedObstacle};
use rs_cam_core::session::SetupData;

use crate::render::LineVertex;
use crate::render::colors::{OBSTACLE, OBSTACLE_CLEARANCE, OBSTACLE_PLANE, OBSTACLE_SELECTED};
use crate::render::fixture_render::{push_prism_lines, push_ring_lines};
use crate::state::AppState;
use crate::state::job::SetupFrame;
use crate::state::selection::Selection;

/// The margin (mm) of the obstacle plane rectangle round the stock and the
/// footprints. A display choice: the rectangle must read as a plane, not
/// as an outline of one of them.
const PLANE_MARGIN_MM: f64 = 10.0;

/// Push the obstacle lines of the setup `sd`, drawn in `frame`, into the
/// line buffer. Nothing when the Obstacles overlay is off.
///
/// The plane draws only while the setup or one of its obstacles is
/// selected (D12): it is the height the tool crosses an obstacle at.
pub(super) fn push_obstacle_lines(
    state: &AppState,
    frame: &SetupFrame,
    sd: &SetupData,
    out: &mut Vec<LineVertex>,
) {
    if !state.viewport.show_obstacles {
        return;
    }
    let stock = state.session().stock_config();
    let models = state.session().models();
    let shift_p = frame.emission_to_display_shift(stock);
    let shift = [shift_p.x, shift_p.y, shift_p.z];
    let setup_id = sd.id;

    let mut resolved: Vec<ResolvedObstacle> = Vec::new();
    for obstacle in sd.obstacles.iter().filter(|o| o.enabled) {
        let Ok(set) = ObstacleSet::resolve(std::slice::from_ref(obstacle), models) else {
            continue;
        };
        let selected = state.selection == Selection::Obstacle(setup_id, obstacle.id);
        let rgb = if selected {
            OBSTACLE_SELECTED
        } else {
            OBSTACLE
        };
        for one in set.obstacles() {
            for polygon in &one.outline {
                push_prism_lines(
                    out,
                    &polygon.exterior,
                    one.outline_bottom_z,
                    one.outline_top_z,
                    rgb,
                    shift,
                );
            }
            for polygon in &one.footprint {
                push_ring_lines(out, &polygon.exterior, one.top_z, OBSTACLE_CLEARANCE, shift);
            }
        }
        resolved.extend(set.obstacles().iter().cloned());
    }

    let focused = match state.selection {
        Selection::Setup(id) | Selection::Obstacle(id, _) => id == setup_id,
        _ => false,
    };
    if !focused || resolved.is_empty() {
        return;
    }
    let set = ObstacleSet::from_resolved(resolved);
    let Some(plane_z) = set.plane_z() else {
        return;
    };
    // The plane covers the displayed stock and every grown footprint.
    let (w, d, _) = frame.effective_stock(stock);
    let mut xy = [0.0_f64, 0.0, w, d];
    for polygon in set.footprints() {
        for v in &polygon.exterior {
            let (x, y) = (v.x + shift[0], v.y + shift[1]);
            xy = [xy[0].min(x), xy[1].min(y), xy[2].max(x), xy[3].max(y)];
        }
    }
    let m = PLANE_MARGIN_MM;
    // The ring is in the display frame already: no shift in X and Y.
    let ring = [
        rs_cam_core::geo::P2::new(xy[0] - m, xy[1] - m),
        rs_cam_core::geo::P2::new(xy[2] + m, xy[1] - m),
        rs_cam_core::geo::P2::new(xy[2] + m, xy[3] + m),
        rs_cam_core::geo::P2::new(xy[0] - m, xy[3] + m),
    ];
    push_ring_lines(out, &ring, plane_z, OBSTACLE_PLANE, [0.0, 0.0, shift[2]]);
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code; a failed lookup is a failed test."
)]
mod tests {
    use super::*;
    use rs_cam_core::ids::{ModelId, ObstacleId};
    use rs_cam_core::session::{AddObstacleArgs, Command, Obstacle, ObstacleGeometry};

    fn state_with(obstacles: Vec<Obstacle>) -> AppState {
        let mut state = AppState::new();
        assert!(
            !state.session().list_setups().is_empty(),
            "an empty project holds one setup"
        );
        for obstacle in obstacles {
            let _ = state
                .apply(Command::AddObstacle(AddObstacleArgs {
                    setup_index: 0,
                    obstacle: Box::new(obstacle),
                }))
                .expect("the obstacle applies");
        }
        state
    }

    fn lines(state: &AppState) -> Vec<LineVertex> {
        let sd = state.session().list_setups()[0].clone();
        let frame = SetupFrame::of(&sd);
        let mut out = Vec::new();
        push_obstacle_lines(state, &frame, &sd, &mut out);
        out
    }

    fn count(out: &[LineVertex], rgb: [f32; 3]) -> usize {
        out.iter().filter(|v| v.color == rgb).count()
    }

    #[test]
    fn a_box_obstacle_draws_its_prism_its_clearance_and_a_plane_on_focus_k2() {
        let mut state = state_with(vec![Obstacle::new_default_box(ObstacleId::from_raw(0))]);
        state.selection = Selection::None;
        let out = lines(&state);
        assert_eq!(count(&out, OBSTACLE), 24, "the box prism");
        assert!(count(&out, OBSTACLE_CLEARANCE) >= 8, "the clearance ring");
        assert_eq!(count(&out, OBSTACLE_PLANE), 0, "no plane without focus");

        let setup = state.session().list_setups()[0].id;
        state.selection = Selection::Setup(setup);
        let out = lines(&state);
        let plane: Vec<&LineVertex> = out.iter().filter(|v| v.color == OBSTACLE_PLANE).collect();
        assert_eq!(plane.len(), 8, "the plane rectangle");
        let sd = &state.session().list_setups()[0];
        let shift_z = SetupFrame::of(sd)
            .emission_to_display_shift(state.session().stock_config())
            .z;
        let plane_z = ObstacleSet::resolve(&sd.obstacles, state.session().models())
            .unwrap()
            .plane_z()
            .unwrap();
        for v in plane {
            assert!((f64::from(v.position[2]) - (plane_z + shift_z)).abs() < 1e-3);
        }

        state.selection = Selection::Obstacle(setup, ObstacleId::from_raw(0));
        let out = lines(&state);
        assert_eq!(count(&out, OBSTACLE_SELECTED), 24, "the selected prism");
        assert_eq!(count(&out, OBSTACLE), 0);

        state.viewport.show_obstacles = false;
        assert!(
            lines(&state).is_empty(),
            "the overlay flag gates everything"
        );
    }

    #[test]
    fn a_broken_obstacle_does_not_blank_the_others_k2() {
        let good = Obstacle::new_default_box(ObstacleId::from_raw(0));
        let mut state = state_with(vec![good]);
        // The core refuses a missing model on add, so the broken record
        // goes into a COPY of the setup that the test hands to the builder.
        let mut broken = Obstacle::new_default_box(ObstacleId::from_raw(1));
        broken.geometry = ObstacleGeometry::Model {
            model_id: ModelId::from_raw(42),
        };
        assert!(
            ObstacleSet::resolve(std::slice::from_ref(&broken), state.session().models()).is_err(),
            "non-vacuity: the broken record does not resolve"
        );
        let mut sd = state.session().list_setups()[0].clone();
        sd.obstacles.push(broken);
        let frame = SetupFrame::of(&sd);
        state.selection = Selection::None;
        let mut out = Vec::new();
        push_obstacle_lines(&state, &frame, &sd, &mut out);
        assert_eq!(count(&out, OBSTACLE), 24, "the good box still draws");
    }
}
