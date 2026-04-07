use crate::geo::P3;

/// Which face of the stock is oriented upward in this setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FaceUp {
    #[default]
    Top,
    Bottom,
    Front,
    Back,
    Left,
    Right,
}

impl FaceUp {
    pub const ALL: &[FaceUp] = &[
        FaceUp::Top,
        FaceUp::Bottom,
        FaceUp::Front,
        FaceUp::Back,
        FaceUp::Left,
        FaceUp::Right,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            FaceUp::Top => "Top",
            FaceUp::Bottom => "Bottom",
            FaceUp::Front => "Front",
            FaceUp::Back => "Back",
            FaceUp::Left => "Left",
            FaceUp::Right => "Right",
        }
    }

    /// Operator instruction for achieving this orientation from default (Top).
    pub fn flip_instruction(&self) -> &'static str {
        match self {
            FaceUp::Top => "No flip needed",
            FaceUp::Bottom => "Flip 180 deg on X axis",
            FaceUp::Front => "Rotate 90 deg forward on X axis",
            FaceUp::Back => "Rotate 90 deg backward on X axis",
            FaceUp::Left => "Rotate 90 deg left on Y axis",
            FaceUp::Right => "Rotate 90 deg right on Y axis",
        }
    }

    pub fn to_key(&self) -> &'static str {
        match self {
            FaceUp::Top => "top",
            FaceUp::Bottom => "bottom",
            FaceUp::Front => "front",
            FaceUp::Back => "back",
            FaceUp::Left => "left",
            FaceUp::Right => "right",
        }
    }

    pub fn from_key(s: &str) -> Self {
        match s {
            "bottom" => FaceUp::Bottom,
            "front" => FaceUp::Front,
            "back" => FaceUp::Back,
            "left" => FaceUp::Left,
            "right" => FaceUp::Right,
            _ => FaceUp::Top,
        }
    }

    /// Transform a point from world coords to this orientation's local frame.
    pub fn transform_point(&self, p: P3, stock_w: f64, stock_d: f64, stock_h: f64) -> P3 {
        match self {
            FaceUp::Top => p,
            FaceUp::Bottom => P3::new(p.x, stock_d - p.y, stock_h - p.z),
            FaceUp::Front => P3::new(p.x, stock_h - p.z, p.y),
            FaceUp::Back => P3::new(p.x, p.z, stock_d - p.y),
            FaceUp::Left => P3::new(stock_h - p.z, p.y, p.x),
            FaceUp::Right => P3::new(p.z, p.y, stock_w - p.x),
        }
    }

    /// Inverse transform: from this orientation's local frame back to world coords.
    pub fn inverse_transform_point(&self, p: P3, stock_w: f64, stock_d: f64, stock_h: f64) -> P3 {
        match self {
            FaceUp::Top => p,
            // Bottom: (x, D-y, H-z) is self-inverse
            FaceUp::Bottom => P3::new(p.x, stock_d - p.y, stock_h - p.z),
            // Front forward: (x, H-z, y) -> inverse: (x, z, H-y)
            FaceUp::Front => P3::new(p.x, p.z, stock_h - p.y),
            // Back forward: (x, z, D-y) -> inverse: (x, D-z, y)
            FaceUp::Back => P3::new(p.x, stock_d - p.z, p.y),
            // Left forward: (H-z, y, x) -> inverse: (z, y, H-x)
            FaceUp::Left => P3::new(p.z, p.y, stock_h - p.x),
            // Right forward: (z, y, W-x) -> inverse: (W-z, y, x)
            FaceUp::Right => P3::new(stock_w - p.z, p.y, p.x),
        }
    }

    /// Effective stock dimensions (W', D', H') after this face-up transform.
    pub fn effective_stock(&self, w: f64, d: f64, h: f64) -> (f64, f64, f64) {
        match self {
            FaceUp::Top | FaceUp::Bottom => (w, d, h),
            FaceUp::Front | FaceUp::Back => (w, h, d),
            FaceUp::Left | FaceUp::Right => (h, d, w),
        }
    }
}

/// Rotation of the stock about the vertical (Z) axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ZRotation {
    #[default]
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl ZRotation {
    pub const ALL: &[ZRotation] = &[
        ZRotation::Deg0,
        ZRotation::Deg90,
        ZRotation::Deg180,
        ZRotation::Deg270,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ZRotation::Deg0 => "0 deg",
            ZRotation::Deg90 => "90 deg",
            ZRotation::Deg180 => "180 deg",
            ZRotation::Deg270 => "270 deg",
        }
    }

    pub fn to_key(&self) -> &'static str {
        match self {
            ZRotation::Deg0 => "0",
            ZRotation::Deg90 => "90",
            ZRotation::Deg180 => "180",
            ZRotation::Deg270 => "270",
        }
    }

    pub fn from_key(s: &str) -> Self {
        match s {
            "90" => ZRotation::Deg90,
            "180" => ZRotation::Deg180,
            "270" => ZRotation::Deg270,
            _ => ZRotation::Deg0,
        }
    }

    /// Transform a point's XY coords by Z rotation in the setup frame.
    pub fn transform_point(&self, p: P3, eff_w: f64, eff_d: f64) -> P3 {
        match self {
            ZRotation::Deg0 => p,
            ZRotation::Deg90 => P3::new(eff_d - p.y, p.x, p.z),
            ZRotation::Deg180 => P3::new(eff_w - p.x, eff_d - p.y, p.z),
            ZRotation::Deg270 => P3::new(p.y, eff_w - p.x, p.z),
        }
    }

    /// Inverse transform: from rotated frame back to the pre-rotation frame.
    pub fn inverse_transform_point(&self, p: P3, eff_w: f64, eff_d: f64) -> P3 {
        match self {
            ZRotation::Deg0 => p,
            // Forward 90: (D-y, x, z) -> inverse is 270: (y, D'-x, z)
            // where D' is the rotated D = original W
            ZRotation::Deg90 => P3::new(p.y, eff_d - p.x, p.z),
            // 180 is self-inverse: (W-x, D-y, z)
            ZRotation::Deg180 => P3::new(eff_w - p.x, eff_d - p.y, p.z),
            // Forward 270: (y, W-x, z) -> inverse: (W-p.y, p.x, z)
            ZRotation::Deg270 => P3::new(eff_w - p.y, p.x, p.z),
        }
    }

    /// Effective stock dims after Z rotation (swaps W and D for 90/270).
    pub fn effective_stock(&self, w: f64, d: f64, h: f64) -> (f64, f64, f64) {
        match self {
            ZRotation::Deg0 | ZRotation::Deg180 => (w, d, h),
            ZRotation::Deg90 | ZRotation::Deg270 => (d, w, h),
        }
    }
}

// ── SetupTransformInfo ────────────────────────────────────────────────

use crate::dexel_stock::StockCutDirection;
use crate::toolpath::{Move, MoveType, Toolpath};

/// Information needed to transform a setup's local coordinates to the global
/// stock frame (inverse of the setup transform).
#[derive(Clone)]
pub struct SetupTransformInfo {
    pub face_up: FaceUp,
    pub z_rotation: ZRotation,
    pub stock_x: f64,
    pub stock_y: f64,
    pub stock_z: f64,
}

impl SetupTransformInfo {
    /// Transform a point from setup-local coordinates to global stock coordinates.
    pub fn local_to_global(&self, p: P3) -> P3 {
        let (eff_w, eff_d, _) =
            self.face_up
                .effective_stock(self.stock_x, self.stock_y, self.stock_z);
        let unrotated = self.z_rotation.inverse_transform_point(p, eff_w, eff_d);
        self.face_up
            .inverse_transform_point(unrotated, self.stock_x, self.stock_y, self.stock_z)
    }

    /// Derive the stock cut direction for this setup (used by playback).
    pub fn cut_direction(&self) -> StockCutDirection {
        match self.face_up {
            FaceUp::Top => StockCutDirection::FromTop,
            FaceUp::Bottom => StockCutDirection::FromBottom,
            FaceUp::Front => StockCutDirection::FromFront,
            FaceUp::Back => StockCutDirection::FromBack,
            FaceUp::Left => StockCutDirection::FromLeft,
            FaceUp::Right => StockCutDirection::FromRight,
        }
    }

    /// Whether this setup requires a transform (non-identity orientation).
    pub fn needs_transform(&self) -> bool {
        self.face_up != FaceUp::Top || self.z_rotation != ZRotation::Deg0
    }

    /// Transform a toolpath from setup-local to global stock frame.
    /// Used for playback data (which needs global-frame toolpaths).
    pub fn transform_toolpath(&self, toolpath: &Toolpath) -> Toolpath {
        let xform = |p: P3| -> P3 { self.local_to_global(p) };

        // Direction transform for arc offsets (linear part only).
        let o_g = xform(P3::new(0.0, 0.0, 0.0));
        let dir_xform = |di: f64, dj: f64| -> (f64, f64) {
            let p_g = xform(P3::new(di, dj, 0.0));
            (p_g.x - o_g.x, p_g.y - o_g.y)
        };

        // Detect reflection (negative determinant -> flip arc direction).
        let ex_g = xform(P3::new(1.0, 0.0, 0.0));
        let ey_g = xform(P3::new(0.0, 1.0, 0.0));
        let det = (ex_g.x - o_g.x) * (ey_g.y - o_g.y) - (ex_g.y - o_g.y) * (ey_g.x - o_g.x);
        let flip_arcs = det < 0.0;

        let new_moves: Vec<Move> = toolpath
            .moves
            .iter()
            .map(|m| {
                let target = xform(m.target);
                let move_type = match m.move_type {
                    MoveType::Rapid => MoveType::Rapid,
                    MoveType::Linear { feed_rate } => MoveType::Linear { feed_rate },
                    MoveType::ArcCW { i, j, feed_rate } => {
                        let (ni, nj) = dir_xform(i, j);
                        if flip_arcs {
                            MoveType::ArcCCW {
                                i: ni,
                                j: nj,
                                feed_rate,
                            }
                        } else {
                            MoveType::ArcCW {
                                i: ni,
                                j: nj,
                                feed_rate,
                            }
                        }
                    }
                    MoveType::ArcCCW { i, j, feed_rate } => {
                        let (ni, nj) = dir_xform(i, j);
                        if flip_arcs {
                            MoveType::ArcCW {
                                i: ni,
                                j: nj,
                                feed_rate,
                            }
                        } else {
                            MoveType::ArcCCW {
                                i: ni,
                                j: nj,
                                feed_rate,
                            }
                        }
                    }
                };
                Move { target, move_type }
            })
            .collect();

        Toolpath { moves: new_moves }
    }
}
