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
    ///
    /// These describe the **physical motion**, so they move with the
    /// transform: the lateral four were swapped in pairs alongside
    /// [`Self::transform_point`] under G-FRONTNAME. To bring the front (−Y)
    /// face up you tip the blank *away* from you; the old text said
    /// "forward", which was correct for the old arm that brought +Y up.
    pub fn flip_instruction(&self) -> &'static str {
        match self {
            FaceUp::Top => "No flip needed",
            FaceUp::Bottom => "Flip 180 deg on X axis",
            FaceUp::Front => "Rotate 90 deg backward on X axis",
            FaceUp::Back => "Rotate 90 deg forward on X axis",
            FaceUp::Left => "Rotate 90 deg right on Y axis",
            FaceUp::Right => "Rotate 90 deg left on Y axis",
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
    ///
    /// # Which world face each name means — G-FRONTNAME
    ///
    /// The lateral four follow the **drafting convention**, ruled by the
    /// operator on 2026-08-22 and pinned by
    /// `tests/face_up_names_follow_drafting_convention_g_frontname.rs`:
    ///
    /// | variant | world face brought up |
    /// |---|---|
    /// | `Front` | −Y |
    /// | `Back`  | +Y |
    /// | `Left`  | −X |
    /// | `Right` | +X |
    ///
    /// That is what every CAD package means by those words, and it is
    /// already what the composite screenshot renderer's panel labels say
    /// (front = the −Y eye, rear = +Y, left = −X, right = +X).
    ///
    /// Until 2026-08-22 all four arms picked the **opposite** face — a
    /// `face_up = "front"` pocket landed on the +Y face and rendered in the
    /// composite's REAR panels, which is how an operator caught it. The
    /// four arms were swapped in pairs (Front↔Back, Left↔Right) in this
    /// method and in [`Self::inverse_transform_point`] together;
    /// [`Self::effective_stock`] is unaffected, because a pair shares its
    /// axis permutation and differs only in which end of it is up.
    pub fn transform_point(&self, p: P3, stock_w: f64, stock_d: f64, stock_h: f64) -> P3 {
        match self {
            FaceUp::Top => p,
            FaceUp::Bottom => P3::new(p.x, stock_d - p.y, stock_h - p.z),
            // Front: local +Z is world −Y, so the −Y face is up.
            FaceUp::Front => P3::new(p.x, p.z, stock_d - p.y),
            // Back: local +Z is world +Y.
            FaceUp::Back => P3::new(p.x, stock_h - p.z, p.y),
            // Left: local +Z is world −X, so the −X face is up.
            FaceUp::Left => P3::new(p.z, p.y, stock_w - p.x),
            // Right: local +Z is world +X.
            FaceUp::Right => P3::new(stock_h - p.z, p.y, p.x),
        }
    }

    /// Inverse transform: from this orientation's local frame back to world coords.
    ///
    /// Each arm is the exact inverse of the same-named arm of
    /// [`Self::transform_point`] — see the G-FRONTNAME note there for which
    /// world face each name picks.
    pub fn inverse_transform_point(&self, p: P3, stock_w: f64, stock_d: f64, stock_h: f64) -> P3 {
        match self {
            FaceUp::Top => p,
            // Bottom: (x, D-y, H-z) is self-inverse
            FaceUp::Bottom => P3::new(p.x, stock_d - p.y, stock_h - p.z),
            // Front forward: (x, z, D-y) -> inverse: (x, D-z, y)
            FaceUp::Front => P3::new(p.x, stock_d - p.z, p.y),
            // Back forward: (x, H-z, y) -> inverse: (x, z, H-y)
            FaceUp::Back => P3::new(p.x, p.z, stock_h - p.y),
            // Left forward: (z, y, W-x) -> inverse: (W-z, y, x)
            FaceUp::Left => P3::new(stock_w - p.z, p.y, p.x),
            // Right forward: (H-z, y, x) -> inverse: (z, y, H-x)
            FaceUp::Right => P3::new(p.z, p.y, stock_h - p.x),
        }
    }

    /// `true` for the four side faces — the ones whose work plane is
    /// **perpendicular** to the world XY plane a 2D drawing is authored in.
    ///
    /// `Top` and `Bottom` are parallel to it (identity and mirror
    /// respectively), so a drawing has a mapping onto them. The lateral
    /// four do not, which is the whole of the work-plane rule — see
    /// [`SetupTransformInfo::drawing_to_local`].
    pub fn is_lateral(&self) -> bool {
        matches!(
            self,
            FaceUp::Front | FaceUp::Back | FaceUp::Left | FaceUp::Right
        )
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
use crate::geo::P2;
use crate::mesh::TriangleMesh;
use crate::polygon::Polygon2;
use crate::toolpath::{Move, MoveType, Toolpath};

/// Information needed to transform a setup's local coordinates to the global
/// stock frame (inverse of the setup transform).
#[derive(Clone, Default)]
pub struct SetupTransformInfo {
    pub face_up: FaceUp,
    pub z_rotation: ZRotation,
    pub stock_x: f64,
    pub stock_y: f64,
    pub stock_z: f64,
    /// Stock origin in world coordinates. Forward transforms (world → local)
    /// subtract this before applying face/rotation. Inverse transforms
    /// (`local_to_global`) operate in stock-relative coordinates and do not
    /// re-add origin; callers that need true world coordinates should add it
    /// themselves.
    pub stock_origin_x: f64,
    pub stock_origin_y: f64,
    pub stock_origin_z: f64,
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

    /// Transform a point from world coordinates to setup-local coordinates.
    ///
    /// Chain: 1) translate by `-stock_origin`, 2) face-up flip,
    /// 3) Z-rotation.
    pub fn world_to_local(&self, p: P3) -> P3 {
        let rel = P3::new(
            p.x - self.stock_origin_x,
            p.y - self.stock_origin_y,
            p.z - self.stock_origin_z,
        );
        let flipped = self
            .face_up
            .transform_point(rel, self.stock_x, self.stock_y, self.stock_z);
        let (eff_w, eff_d, _) =
            self.face_up
                .effective_stock(self.stock_x, self.stock_y, self.stock_z);
        self.z_rotation.transform_point(flipped, eff_w, eff_d)
    }

    /// Transform a triangle mesh from world coordinates to setup-local coordinates.
    pub fn apply_to_mesh(&self, mesh: &TriangleMesh) -> TriangleMesh {
        let new_verts: Vec<P3> = mesh
            .vertices
            .iter()
            .map(|v| self.world_to_local(*v))
            .collect();
        TriangleMesh::from_raw(new_verts, mesh.triangles.clone())
    }

    /// Move ONE point of a **drawing** — SVG/DXF artwork, or a target picked
    /// off it — from the frame it was authored in into this setup's work
    /// plane.
    ///
    /// # The rule (operator ruling, 2026-08-22)
    ///
    /// > A 2D drawing is consumed in the **work plane of the setup that
    /// > uses it.**
    ///
    /// That is already what ships for the faces whose work plane is
    /// parallel to the drawing plane, and this method is those cases
    /// unchanged:
    ///
    /// | face | work plane vs drawing plane | what happens |
    /// |---|---|---|
    /// | `Top` / `Deg0` | same | identity — callers skip the transform entirely (`needs_transform()` is false) |
    /// | `Top` / `Deg90` etc. | same, rotated | rotated in-plane |
    /// | `Bottom` | same plane, other side | mirrored — how a feature registers to the same physical place |
    /// | `Front`/`Back`/`Left`/`Right` | **perpendicular** | the lateral branch below |
    ///
    /// For the lateral four no mapping exists. `world_to_local` computes a
    /// perfectly correct orthographic projection of a horizontal drawing
    /// onto a vertical face — and that projection is a **line**. The
    /// arithmetic was never wrong; the request was meaningless
    /// (G-SIDEFACE-POLYCOLLAPSE). So the drawing's coordinates are taken
    /// as the work plane's own coordinates: no origin subtraction, no face
    /// projection.
    ///
    /// `z_rotation` still applies, in-plane, against the lateral face's
    /// **effective** stock dimensions — exactly what `Top`/`Deg90` does to
    /// a drawing in its plane, and the same `(eff_w, eff_d)` pair
    /// [`Self::effective_stock_bbox`] reports, so a drawing inside the
    /// work plane stays inside it.
    ///
    /// **This is for drawings only.** World-anchored geometry — fixture and
    /// keep-out footprints, which describe hardware bolted to the table —
    /// must NOT be reinterpreted this way; see [`Self::apply_to_polygons`].
    pub fn drawing_to_local(&self, p: P2) -> P2 {
        if self.face_up.is_lateral() {
            let (eff_w, eff_d, _) =
                self.face_up
                    .effective_stock(self.stock_x, self.stock_y, self.stock_z);
            let rotated = self
                .z_rotation
                .transform_point(P3::new(p.x, p.y, 0.0), eff_w, eff_d);
            P2::new(rotated.x, rotated.y)
        } else {
            let local = self.world_to_local(P3::new(p.x, p.y, 0.0));
            P2::new(local.x, local.y)
        }
    }

    /// Transform a model's **drawing** polygons into this setup's work
    /// plane, per [`Self::drawing_to_local`].
    ///
    /// Use this for anything that came out of an SVG/DXF/STEP-face import
    /// and describes the *part*. Use [`Self::apply_to_polygons`] for
    /// world-frame footprints instead — the two differ only on lateral
    /// setups, and that difference is the whole point.
    ///
    /// Closed rings come back **re-wound to the crate convention** (exterior
    /// CCW, holes CW). Open paths keep their point order, because for them
    /// the order is the machining direction.
    pub fn apply_to_drawing_polygons(&self, polygons: &[Polygon2]) -> Vec<Polygon2> {
        self.map_polygons(polygons, |p| self.drawing_to_local(p))
    }

    /// Transform **world-frame** 2D polygons to setup-local XY coordinates.
    ///
    /// This is the orthographic projection of a horizontal world-XY shape
    /// into the setup frame, and it is the right answer for geometry that
    /// is genuinely anchored in the world: fixture and keep-out footprints,
    /// which describe clamps and no-go zones fixed to the machine table.
    /// Their placement does not follow the part when the part is turned on
    /// its side.
    ///
    /// It is the WRONG answer for a drawing — on a lateral setup the
    /// projection is a degenerate line — which is why
    /// [`Self::apply_to_drawing_polygons`] exists as a separate door. On
    /// `Top` and `Bottom` the two doors agree exactly; they part company
    /// only on `Front`/`Back`/`Left`/`Right`.
    ///
    /// Note that on a lateral setup this projection collapses a footprint
    /// to a line too, which would silently DELETE a keep-out. Generation
    /// refuses that combination upstream rather than letting it through —
    /// see `ProjectSession::check_lateral_setup_support` (G-LATERALKEEPOUT).
    ///
    /// Closed rings come back **re-wound to the crate convention** (exterior
    /// CCW, holes CW) — see the winding note inside. Open paths keep their
    /// point order, because for them the order is the machining direction.
    pub fn apply_to_polygons(&self, polygons: &[Polygon2]) -> Vec<Polygon2> {
        self.map_polygons(polygons, |p| {
            let p3 = self.world_to_local(P3::new(p.x, p.y, 0.0));
            P2::new(p3.x, p3.y)
        })
    }

    /// Shared body of the two polygon doors: per-point mapping plus the
    /// open/closed and winding invariants, which are identical for both
    /// and must stay that way.
    fn map_polygons(&self, polygons: &[Polygon2], point: impl Fn(P2) -> P2) -> Vec<Polygon2> {
        polygons
            .iter()
            .map(|poly| {
                let ext: Vec<P2> = poly.exterior.iter().map(|p| point(*p)).collect();
                let holes: Vec<Vec<P2>> = poly
                    .holes
                    .iter()
                    .map(|hole| hole.iter().map(|p| point(*p)).collect())
                    .collect();
                // Preserve the open/closed flag: `Polygon2::with_holes` forces
                // closed=true which would silently re-close open paths (rivers,
                // traces) and make project_curve emit a phantom segment from
                // the path's end back to its start.
                let mut result = Polygon2::with_holes(ext, holes);
                result.closed = poly.closed;
                // G-PROFILE-FLIP: a face-up flip is a MIRROR in XY — the
                // `FaceUp::Bottom` row of `transform_point` maps (x, y) to
                // (x, D - y), determinant -1 — so every ring comes out of the
                // loop above wound the opposite way. Nothing downstream
                // re-normalises: the importers are the ones that establish
                // "exterior CCW, holes CW" (`svg_input` and `dxf_input` both
                // call `ensure_winding` on load) and this is the only place
                // that then breaks it.
                //
                // It is not cosmetic, because cavalier's offset sign is
                // defined against the direction of travel — positive offsets
                // to the LEFT of the segment tangent — so on a reversed ring
                // every offset in the 2.5D stack silently inverts. Observed:
                // an Outside profile on a Bottom setup came out INSET by the
                // tool radius, i.e. cutting the part away. `Shape` has the
                // same dependency by another route (it classifies CCW plines
                // as boundaries and CW as holes), so a flipped pocket
                // exterior would be taken for a hole.
                //
                // Fixing it here rather than in each offset consumer keeps
                // one invariant with one owner. Closed rings only: an open
                // path (river, trace) has no winding to speak of and its
                // point order IS the machining direction.
                //
                // The lateral DRAWING branch is a proper rotation
                // (determinant +1), so it never flips a ring and this call
                // is a no-op there. It stays unconditional anyway: the
                // invariant has one owner, and an owner that only runs on
                // the mappings someone remembered to list is not one.
                if result.closed {
                    result.ensure_winding();
                }
                result
            })
            .collect()
    }

    /// Compute the effective stock bounding box in setup-local coordinates.
    /// After the face-up + Z-rotation transform, stock occupies the axis-aligned
    /// box from (0,0,0) to `(eff_w, eff_d, eff_h)`.
    pub fn effective_stock_bbox(&self) -> crate::geo::BoundingBox3 {
        let (w, d, h) = self
            .face_up
            .effective_stock(self.stock_x, self.stock_y, self.stock_z);
        let (eff_w, eff_d, eff_h) = self.z_rotation.effective_stock(w, d, h);
        crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(eff_w, eff_d, eff_h),
        }
    }

    /// Derive the stock cut direction for this setup (used by playback).
    /// The direction the tool advances in, **in the stock-relative global
    /// frame**, for a setup in this orientation.
    ///
    /// # Why this is an identity, and why that is not the same claim it was
    ///
    /// The two names still describe **opposite ends of the same setup**, and
    /// that has not changed:
    ///
    /// * `FaceUp::Front` names *the face that is up* — pointing at the
    ///   spindle.
    /// * `StockCutDirection::FromFront` names *the side the tool arrives
    ///   from*, and its own doc pins that as the −Y side.
    ///
    /// The mapping between them is whatever `inverse_transform_point` makes
    /// it, and nothing else. Today, with `FaceUp::Front` bringing the world
    /// **−Y** face up, local `+Z` maps to global `−Y`: the tool arrives from
    /// −Y, which is `FromFront`. All six arms come out as the identity.
    ///
    /// ## Two defects, in order — do not read this as "it was always fine"
    ///
    /// **G-LATERALSIGN** (fixed first): the lateral arms *were* an identity,
    /// written by assuming the names matched, against a transform for which
    /// they did not. `FaceUp::Front` then brought the **+Y** face up, so the
    /// tool arrived from +Y and the correct answer was `FromBack`. The arms
    /// were changed to that negation, and it was right for the transform as
    /// it stood.
    ///
    /// **G-FRONTNAME** (fixed second, 2026-08-22): the transform itself was
    /// picking the wrong world face on all four laterals — `Front` machined
    /// +Y, drafting's *back*, contradicting the composite renderer's panel
    /// labels and every CAD package. The operator ruled that the drafting
    /// convention wins, so [`FaceUp::transform_point`] swapped Front↔Back and
    /// Left↔Right. That returned this mapping to the identity.
    ///
    /// So the identity is back, but it is not the identity that was here
    /// before: that one was *assumed*, this one is *derived*.
    /// `tests/cut_direction_matches_transform_g_lateralsign.rs` is what makes
    /// the difference real — it transcribes no table, it pushes points
    /// through `inverse_transform_point` and derives the required sign, with
    /// the two Z faces as the control on the derivation. It went red between
    /// the two halves of the G-FRONTNAME fix and is what dictated this table.
    ///
    /// Scope of the G-LATERALSIGN damage: this feeds only the **global
    /// playback stock**. Metrics, gates, collisions and checkpoint meshes are
    /// computed on the per-setup `group_stock`, which `compute/simulate.rs`
    /// stamps with a hardcoded `FromTop` because setup-local Z always is the
    /// tool axis. So the wrong sign never moved a number an operator reads;
    /// it removed material from the far face instead of the near one in the
    /// live-scrub viewport.
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

    /// Whether this setup inverts the Z axis (i.e. `FaceUp::Bottom`).
    ///
    /// This is the single source of truth for project_curve's `setup_z_flipped`
    /// flag — if the setup transform has already Z-inverted the mesh, the
    /// operation must not apply its own Z flip.
    pub fn is_z_flipped(&self) -> bool {
        matches!(self.face_up, FaceUp::Bottom)
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
                Move {
                    target,
                    move_type,
                    intent: m.intent,
                }
            })
            .collect();

        Toolpath { moves: new_moves }
    }
}
