//! `StockCutDirection` — which side of the stock the tool approaches from,
//! and how a 3-D point decomposes into the corresponding grid's
//! `(u, v, depth)` triple.

use crate::dexel::DexelAxis;

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
    pub(super) fn decompose(self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        match self.grid_axis() {
            DexelAxis::Z => (x, y, z), // Z-grid: u=X, v=Y, depth=Z
            DexelAxis::Y => (x, z, y), // Y-grid: u=X, v=Z, depth=Y
            DexelAxis::X => (y, z, x), // X-grid: u=Y, v=Z, depth=X
        }
    }
}
