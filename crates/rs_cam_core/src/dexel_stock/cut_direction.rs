//! `StockCutDirection` — which side of the stock the tool approaches from,
//! and how a 3-D point decomposes into the corresponding grid's
//! `(u, v, depth)` triple.

use crate::stock::dexel::DexelAxis;

// ── Cut direction ───────────────────────────────────────────────────────

/// Which side of the stock the tool approaches from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StockCutDirection {
    /// Tool enters from above (+Z) — removes material above the cutter surface (Z-grid).
    FromTop,
    /// Tool enters from below (−Z) — removes material below the cutter surface (Z-grid).
    FromBottom,
    /// Tool enters from the front face (−Y side) — stamps on Y-grid.
    FromFront,
    /// Tool enters from the back face (+Y side) — stamps on Y-grid.
    FromBack,
    /// Tool enters from the left face (−X side) — stamps on X-grid.
    FromLeft,
    /// Tool enters from the right face (+X side) — stamps on X-grid.
    FromRight,
}

impl StockCutDirection {
    /// Which grid axis this direction stamps on.
    pub fn grid_axis(self) -> DexelAxis {
        match self {
            Self::FromTop | Self::FromBottom => DexelAxis::Z,
            Self::FromFront | Self::FromBack => DexelAxis::Y,
            Self::FromLeft | Self::FromRight => DexelAxis::X,
        }
    }

    /// Whether the tool enters from the high side of the ray axis.
    ///
    /// High-side entry removes material via `subtract_above`;
    /// low-side entry removes material via `subtract_below`.
    pub fn cuts_from_high_side(self) -> bool {
        match self {
            Self::FromTop | Self::FromBack | Self::FromRight => true,
            Self::FromBottom | Self::FromFront | Self::FromLeft => false,
        }
    }

    /// Decompose a 3-D point `(x, y, z)` into `(grid_u, grid_v, ray_depth)`
    /// for the grid axis this direction stamps on.
    ///
    /// STK-02: the permutation table lives on [`DexelAxis::decompose`], which
    /// `DexelGrid::from_bounds` also reads. One table, so a grid origin and a
    /// stamped point cannot disagree.
    pub(super) fn decompose(self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        self.grid_axis().decompose(x, y, z)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::geo::{BoundingBox3, P3};
    use crate::stock::dexel::DexelGrid;

    const DIRECTIONS: [StockCutDirection; 6] = [
        StockCutDirection::FromTop,
        StockCutDirection::FromBottom,
        StockCutDirection::FromFront,
        StockCutDirection::FromBack,
        StockCutDirection::FromLeft,
        StockCutDirection::FromRight,
    ];

    /// STK-02 round trip: the grid constructor and `decompose` read one
    /// permutation table, so for every direction the grid origin IS the
    /// decomposed low corner and the ray span IS the decomposed depth range.
    ///
    /// Teeth: swap two arms of `DexelAxis::decompose` and this test goes red
    /// on the first direction whose axis moved.
    #[test]
    fn grid_origin_matches_decompose_for_every_direction() {
        // Deliberately unequal extents, so a swapped permutation changes the
        // numbers instead of cancelling.
        let bbox = BoundingBox3 {
            min: P3::new(-3.0, -7.0, -11.0),
            max: P3::new(5.0, 13.0, 29.0),
        };
        for direction in DIRECTIONS {
            let axis = direction.grid_axis();
            let grid = DexelGrid::from_bounds(&bbox, 1.0, axis);
            let (u_min, v_min, depth_min) = direction.decompose(bbox.min.x, bbox.min.y, bbox.min.z);
            let (u_max, v_max, depth_max) = direction.decompose(bbox.max.x, bbox.max.y, bbox.max.z);

            // The table itself, written out independently. Without this the
            // test only proves the two readers agree, so a swapped pair of
            // arms would move both sides together and stay green.
            let expected = match axis {
                DexelAxis::Z => (bbox.min.x, bbox.min.y, bbox.min.z),
                DexelAxis::Y => (bbox.min.x, bbox.min.z, bbox.min.y),
                DexelAxis::X => (bbox.min.y, bbox.min.z, bbox.min.x),
            };
            assert_eq!(
                (u_min, v_min, depth_min),
                expected,
                "{direction:?}: decompose table"
            );

            assert_eq!(grid.axis, axis, "{direction:?}: grid axis");
            assert!(
                (grid.origin_u - u_min).abs() < 1e-12,
                "{direction:?}: origin_u {} != {u_min}",
                grid.origin_u
            );
            assert!(
                (grid.origin_v - v_min).abs() < 1e-12,
                "{direction:?}: origin_v {} != {v_min}",
                grid.origin_v
            );

            let expected_cols = ((u_max - u_min) / 1.0).ceil() as usize + 1;
            let expected_rows = ((v_max - v_min) / 1.0).ceil() as usize + 1;
            assert_eq!(grid.cols, expected_cols, "{direction:?}: cols");
            assert_eq!(grid.rows, expected_rows, "{direction:?}: rows");

            let ray = grid.ray(0, 0);
            assert_eq!(ray.len(), 1, "{direction:?}: one segment per ray");
            let seg = ray[0];
            assert!(
                (f64::from(seg.enter) - depth_min).abs() < 1e-6,
                "{direction:?}: segment entry {} != {depth_min}",
                seg.enter
            );
            assert!(
                (f64::from(seg.exit) - depth_max).abs() < 1e-6,
                "{direction:?}: segment exit {} != {depth_max}",
                seg.exit
            );
            assert!(
                (f64::from(grid.conservative_top_at(0, 0)) - depth_max).abs() < 1e-6,
                "{direction:?}: conservative top != depth max"
            );
        }
    }

    /// The three named constructors stay one-line wrappers: each must build
    /// exactly what `from_bounds` builds for its own axis.
    #[test]
    fn the_three_axis_wrappers_agree_with_from_bounds() {
        let bbox = BoundingBox3 {
            min: P3::new(-3.0, -7.0, -11.0),
            max: P3::new(5.0, 13.0, 29.0),
        };
        for (axis, wrapper) in [
            (
                DexelAxis::Z,
                DexelGrid::z_grid_from_bounds as fn(&BoundingBox3, f64) -> DexelGrid,
            ),
            (DexelAxis::X, DexelGrid::x_grid_from_bounds),
            (DexelAxis::Y, DexelGrid::y_grid_from_bounds),
        ] {
            let direct = DexelGrid::from_bounds(&bbox, 0.5, axis);
            let wrapped = wrapper(&bbox, 0.5);
            assert_eq!(direct.axis, wrapped.axis, "{axis:?}");
            assert_eq!(direct.rows, wrapped.rows, "{axis:?}");
            assert_eq!(direct.cols, wrapped.cols, "{axis:?}");
            assert_eq!(direct.origin_u, wrapped.origin_u, "{axis:?}");
            assert_eq!(direct.origin_v, wrapped.origin_v, "{axis:?}");
            assert_eq!(direct.cell_size, wrapped.cell_size, "{axis:?}");
        }
    }
}
