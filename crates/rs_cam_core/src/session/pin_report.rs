//! The one report of a pin set (P7, 2026-10-12).
//!
//! The stock panel chips, the pin table, the viewport colours, the
//! wrong-orientation ghost and the MCP `pin_report` all read
//! [`ProjectSession::pin_set_report`]. No surface computes a pin number a
//! second time.

use crate::compute::alignment_pins::{
    AlignmentPin, AxisPairPattern, MixedPairPattern, PIN_MATCH_TOL_MM, PIN_WALL_MM, PinFlipKind,
    PinFlipReport, PinObstacle, PinSetupMap, PinWalls, closest_wrong_seating_images,
    fit_axis_plus_mirror_pair, fit_two_mixed_diameters, pin_findings_for_setups, pin_wall_findings,
    pin_walls, seat_miss_per_pin_mm, validate_pins_for_flip,
};
use crate::compute::transform::{FaceUp, ZRotation};
use crate::ids::ModelId;

use super::ProjectSession;

/// The preset that a stored pin set fits, read back with the `fit_*`
/// functions of `compute::alignment_pins::pattern`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PinPreset {
    /// One pin on the flip axis plus a mirror pair off the axis.
    AxisPlusMirrorPair(AxisPairPattern),
    /// Two pins of different diameters on the flip axis.
    TwoMixedDiameters(MixedPairPattern),
    /// No preset fits the set.
    Custom,
}

/// The state of one pin, for its colour in the viewport and its row in
/// the stock panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinStatus {
    /// Every check passes.
    Good,
    /// A wall is thinner than [`PIN_WALL_MM`].
    ThinWall,
    /// The pin does not seat after the flip, the set does not key, or the
    /// pin is not wholly inside the stock.
    Fails,
}

/// Everything the surfaces say about the stored pin set.
#[derive(Clone)]
pub struct PinSetReport {
    /// The index of the setup the pins are judged against: the first
    /// setup whose flip is in plane. `None` when no such setup exists.
    pub judged_setup: Option<usize>,
    /// The map of the judged setup (a `Top` map when none exists).
    pub map: PinSetupMap,
    /// Seating, keying and bounds.
    pub flip: PinFlipReport,
    /// For each pin, the distance from the dowel to the nearest hole that
    /// it fits after the flip. Empty when the flip is not modelled.
    pub seat_miss_mm: Vec<f64>,
    /// Where each hole lands in the closest wrong seating, stock-local,
    /// in pin order. Empty when no turn of the part seats.
    pub wrong_seating_images: Vec<(f64, f64)>,
    /// The walls of each pin, in pin order.
    pub walls: Vec<PinWalls>,
    /// The models that count as the part ([`ProjectSession::pin_part_models`]).
    pub part_models: Vec<ModelId>,
    /// `true` when the operator chose the part models; `false` for the
    /// default rule.
    pub part_models_chosen: bool,
    /// One row for each pin size of the reported pins: the pin-drill
    /// operation and the tool that drills the holes of the size (R1).
    pub pin_tools: Vec<super::PinToolBinding>,
    /// The obstacles of the wall check, STOCK-LOCAL (X0 Y0 at the stock's
    /// min corner): the part models, the keep-out zones and the fixtures.
    /// The plan view of the stock panel draws these outlines (R3).
    pub obstacles_local: Vec<PinObstacle>,
    /// The preset the set fits. `None` when the judged map has no flip
    /// axis.
    pub preset: Option<PinPreset>,
    /// The warning lines: the flip report's, then the walls and the tool.
    pub findings: Vec<String>,
}

impl PinSetReport {
    /// The smallest wall from a pin to a stock edge.
    #[must_use]
    pub fn wall_to_stock_edge_mm(&self) -> Option<f64> {
        self.walls.iter().map(|w| w.stock_edge_mm).reduce(f64::min)
    }

    /// The smallest wall from a pin to a part model.
    #[must_use]
    pub fn wall_to_part_mm(&self) -> Option<f64> {
        self.walls
            .iter()
            .filter_map(|w| w.part.as_ref().map(|n| n.wall_mm))
            .reduce(f64::min)
    }

    /// The smallest wall from a pin to a keep-out zone or a fixture.
    #[must_use]
    pub fn wall_to_keep_out_mm(&self) -> Option<f64> {
        self.walls
            .iter()
            .filter_map(|w| w.keep_out.as_ref().map(|n| n.wall_mm))
            .reduce(f64::min)
    }

    /// The smallest of all the walls, over every pin.
    #[must_use]
    pub fn min_wall_mm(&self) -> Option<f64> {
        self.walls.iter().map(PinWalls::min_mm).reduce(f64::min)
    }

    /// The state of pin `index` (see [`PinStatus`]).
    ///
    /// A set that does not key fails at every pin: no single pin is the
    /// cause. A pin whose dowel finds no hole after the flip fails, and so
    /// does a pin that is not wholly inside the stock.
    #[must_use]
    pub fn pin_status(&self, index: usize) -> PinStatus {
        let unseated = self
            .seat_miss_mm
            .get(index)
            .is_some_and(|m| *m > PIN_MATCH_TOL_MM);
        let fails = !self.flip.keyed() || unseated || self.flip.out_of_bounds.contains(&index);
        if fails {
            return PinStatus::Fails;
        }
        let thin = self
            .walls
            .get(index)
            .is_some_and(|w| w.min_mm() < PIN_WALL_MM);
        if thin {
            PinStatus::ThinWall
        } else {
            PinStatus::Good
        }
    }
}

impl ProjectSession {
    /// The one report of the stored pin set (see the module doc).
    ///
    /// - The judged setup is the first setup that turns the part over in
    ///   plane (the selector of `compute::alignment_pins::pin_flip_map`).
    /// - Seating, keying, the bar and the closest wrong seating come from
    ///   `validate_pins_for_flip`; the ghost from
    ///   `closest_wrong_seating_images`.
    /// - The walls are to the stock edge, to the part models
    ///   ([`Self::pin_part_models`]), to the keep-out zones and to the
    ///   fixtures. The obstacles are world-frame, the pins stock-local.
    #[must_use]
    pub fn pin_set_report(&self) -> PinSetReport {
        self.pin_set_report_for(&self.stock.alignment_pins)
    }

    /// The report of `pins` on this project's stock, setups, models and
    /// tools. The stock panel passes its draft pins, so the chips follow a
    /// drag before the edit commits; every other reader passes the stored
    /// pins through [`Self::pin_set_report`].
    #[must_use]
    pub fn pin_set_report_for(&self, pins: &[AlignmentPin]) -> PinSetReport {
        let stock = &self.stock;
        let (w, d) = (stock.x, stock.y);
        let judged = self.setups.iter().enumerate().find(|(_, s)| {
            PinSetupMap::new(s.face_up, s.z_rotation, w, d).kind() == PinFlipKind::InPlane
        });
        let (face_up, z_rotation) = judged.map_or((FaceUp::Top, ZRotation::Deg0), |(_, s)| {
            (s.face_up, s.z_rotation)
        });
        let flip = validate_pins_for_flip(pins, face_up, z_rotation, w, d);
        let map = PinSetupMap::new(face_up, z_rotation, w, d);

        let seat_miss_mm = if map.kind() == PinFlipKind::InPlane {
            seat_miss_per_pin_mm(pins, |p| map.map_xy(p.at.x(), p.at.y()))
        } else {
            Vec::new()
        };
        let wrong_seating_images =
            closest_wrong_seating_images(pins).map_or_else(Vec::new, |(_, images)| images);

        let part_models = self.pin_part_models();
        let local: Vec<_> = self
            .pin_obstacles(&part_models)
            .iter()
            .map(|o| o.translated(-stock.origin_x, -stock.origin_y))
            .collect();
        let walls = pins.iter().map(|p| pin_walls(p, w, d, &local)).collect();
        let pin_tools = self.pin_tool_bindings_for(pins);

        // Every setup's findings (a quarter turn on a stock that is not
        // square is judged by no flip map), then the walls and the tool.
        let mut findings: Vec<String> = pin_findings_for_setups(
            pins,
            self.setups.iter().map(|s| (s.face_up, s.z_rotation)),
            w,
            d,
        )
        .into_iter()
        .map(|f| f.text)
        .collect();
        findings.extend(
            pin_wall_findings(pins, w, d, &local)
                .into_iter()
                .chain(self.pin_tool_mismatch_findings(pins))
                .chain(if pins.is_empty() {
                    Vec::new()
                } else {
                    self.pin_obstacle_findings()
                })
                .map(|f| f.text),
        );

        let preset = map.axis_segment().map(|_| {
            if let Some(fit) = fit_axis_plus_mirror_pair(pins, &map) {
                PinPreset::AxisPlusMirrorPair(fit)
            } else if let Some(fit) = fit_two_mixed_diameters(pins, &map) {
                PinPreset::TwoMixedDiameters(fit)
            } else {
                PinPreset::Custom
            }
        });

        PinSetReport {
            judged_setup: judged.map(|(index, _)| index),
            map,
            flip,
            seat_miss_mm,
            wrong_seating_images,
            walls,
            part_models,
            part_models_chosen: stock.pin_part_models.is_some(),
            pin_tools,
            obstacles_local: local,
            preset,
            findings,
        }
    }
}
