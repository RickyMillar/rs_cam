use rs_cam_core::geo::BoundingBox3;
// Serialize/Deserialize not directly needed in this file any more — every
// persisted type it names is defined (and derived) in core.

// ── Re-exports from rs_cam_core::compute (Phase 1 service layer extraction) ──
pub use rs_cam_core::compute::stock_config::{
    AlignmentPin, FixtureId, FlipAxis, KeepOutId, ModelId, ModelKind, ModelUnits, PostConfig,
    PostFormat, SetupId, StockConfig,
};
pub use rs_cam_core::compute::tool_config::{
    BitCutDirection, ToolConfig, ToolId, ToolMaterial, ToolType,
};
pub use rs_cam_core::compute::transform::{FaceUp, ZRotation};

// LoadedModel is now the single core type — viz re-exports it directly.
pub use rs_cam_core::session::LoadedModel;

// ToolConfig, ToolType, ToolMaterial, BitCutDirection, PostConfig, PostFormat,
// StockConfig, AlignmentPin, FlipAxis, ModelId, ToolId, SetupId, FixtureId,
// KeepOutId, ModelKind, ModelUnits are now re-exported from core above.

// FaceUp and ZRotation are now re-exported from core::compute::transform above.

// Setup datum types (`Corner`, `XYDatum`, `ZDatum`, `DatumConfig`) moved to
// `rs_cam_core::session` in W9 / P-2 so the datum could join the project
// schema. This file and `state::runtime` each carried a byte-identical
// private copy; both are now re-exports of the one core definition.
pub use rs_cam_core::session::{Corner, DatumConfig, XYDatum, ZDatum};

// FlipAxis and AlignmentPin are now re-exported from core::compute::stock_config above.

/// The orientation half of a setup: what the transform helpers need.
///
/// **This is a frame descriptor, not a model of a setup.** The setup is
/// `rs_cam_core::session::SetupData`, and it is the only one. This type
/// is `(FaceUp, ZRotation)` with names, so a draw site that has released
/// its borrow of the session can still place geometry.
///
/// C12 deleted the second data model this file used to carry —
/// `JobState`, a `Setup` with fixtures and toolpaths, `Fixture` and
/// `KeepOutZone`. Only the legacy project reader built them, and C01 and
/// C11 deleted that reader. Every live fixture and keep-out edit already
/// builds the CORE type and dispatches a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupFrame {
    pub face_up: FaceUp,
    pub z_rotation: ZRotation,
}

impl SetupFrame {
    /// The frame a setup's orientation names.
    pub fn new(face_up: FaceUp, z_rotation: ZRotation) -> Self {
        Self {
            face_up,
            z_rotation,
        }
    }

    /// The frame of a core setup record.
    pub fn of(setup: &rs_cam_core::session::SetupData) -> Self {
        Self::new(setup.face_up, setup.z_rotation)
    }

    /// Core's setup-transform descriptor for this frame against `stock`.
    ///
    /// The setup-transform algorithm family (world→local point, mesh, and
    /// polygon transforms) lives in
    /// [`rs_cam_core::compute::transform::SetupTransformInfo`] and nowhere
    /// else. This crate used to carry a second copy of all three, and they
    /// had already diverged: the viz copy of the polygon transform re-wound
    /// **open** paths as well as closed rings, so a mirroring setup
    /// (`FaceUp::Bottom`) reversed a river's machining direction on the GUI
    /// compute path while core left it alone (G-POLYTRANSFORM-DUP; the
    /// closed-rings-only rule is G-PROFILE-FLIP, documented at
    /// `SetupTransformInfo::apply_to_polygons`).
    pub fn transform_info(
        &self,
        stock: &StockConfig,
    ) -> rs_cam_core::compute::transform::SetupTransformInfo {
        rs_cam_core::compute::transform::SetupTransformInfo {
            face_up: self.face_up,
            z_rotation: self.z_rotation,
            stock_x: stock.x,
            stock_y: stock.y,
            stock_z: stock.z,
            stock_origin_x: stock.origin_x,
            stock_origin_y: stock.origin_y,
            stock_origin_z: stock.origin_z,
        }
    }

    /// Transform a point from world coords to this setup's local frame.
    /// Translates to stock-relative coords first, then applies FaceUp + ZRotation.
    pub fn transform_point(
        &self,
        p: rs_cam_core::geo::P3,
        stock: &StockConfig,
    ) -> rs_cam_core::geo::P3 {
        self.transform_info(stock).world_to_local(p)
    }

    /// Effective stock dimensions in this setup's local frame.
    pub fn effective_stock(&self, stock: &StockConfig) -> (f64, f64, f64) {
        let (w, d, h) = self.face_up.effective_stock(stock.x, stock.y, stock.z);
        self.z_rotation.effective_stock(w, d, h)
    }

    /// Shift that takes data in this setup's *emission* frame into the
    /// viewport's zero-rooted local display frame.
    ///
    /// Identity setups (Top / 0°) emit toolpaths — and root their dexel
    /// grid (F-024) — in the world frame, which differs from the
    /// zero-rooted "machine view" the viewport displays by exactly
    /// `-stock.origin`. Non-identity setups already emit in the local
    /// frame, so their shift is zero. (Heights/setup-frame audit
    /// 2026-06-12, finding 1: drawing identity toolpaths unshifted made
    /// every rough on an `origin != 0` project render below the mesh —
    /// "through the stock floor" on thick stocks.)
    pub fn emission_to_display_shift(&self, stock: &StockConfig) -> rs_cam_core::geo::P3 {
        use rs_cam_core::geo::P3;
        if self.face_up == FaceUp::Top && self.z_rotation == ZRotation::Deg0 {
            P3::new(-stock.origin_x, -stock.origin_y, -stock.origin_z)
        } else {
            P3::new(0.0, 0.0, 0.0)
        }
    }

    /// Inverse transform: from this setup's local frame back to world coords.
    /// Undoes ZRotation, then FaceUp, then translates back to world coords.
    ///
    /// **Not** a delegate to
    /// [`rs_cam_core::compute::transform::SetupTransformInfo::local_to_global`],
    /// which is a different function despite the matching name: it stops in
    /// *stock-relative* coordinates and deliberately does not re-add the stock
    /// origin (its own doc says so, and `transform_toolpath` depends on that).
    /// This one closes the round trip with `transform_point` in world
    /// coordinates, so the two agree only when the origin is zero.
    pub fn inverse_transform_point(
        &self,
        p: rs_cam_core::geo::P3,
        stock: &StockConfig,
    ) -> rs_cam_core::geo::P3 {
        use rs_cam_core::geo::P3;
        // 1. Undo ZRotation
        let (eff_w, eff_d, _) = self.face_up.effective_stock(stock.x, stock.y, stock.z);
        let unrotated = self.z_rotation.inverse_transform_point(p, eff_w, eff_d);
        // 2. Undo FaceUp flip → stock-relative coords
        let rel = self
            .face_up
            .inverse_transform_point(unrotated, stock.x, stock.y, stock.z);
        // 3. Translate stock-relative → world
        P3::new(
            rel.x + stock.origin_x,
            rel.y + stock.origin_y,
            rel.z + stock.origin_z,
        )
    }

    /// Whether this setup requires geometry transforms (non-identity orientation).
    pub fn needs_transform(&self) -> bool {
        self.face_up != FaceUp::Top || self.z_rotation != ZRotation::Deg0
    }
}

impl Default for SetupFrame {
    fn default() -> Self {
        Self::new(FaceUp::default(), ZRotation::default())
    }
}

// ── Helpers for session fixture/keep-out bbox computation ──
// The core session `Fixture` and `KeepOutZone` lack bbox methods, so we
// provide free functions here that mirror the viz-local equivalents.

/// Transform an axis-aligned bbox through a setup transform by mapping all 8
/// corners to the setup-local frame and taking min/max. face_up + z_rotation
/// are 90° increments so the result remains axis-aligned.
fn setup_local_bbox(
    bbox: &BoundingBox3,
    info: &rs_cam_core::compute::transform::SetupTransformInfo,
) -> BoundingBox3 {
    use rs_cam_core::geo::P3;
    let corners = [
        P3::new(bbox.min.x, bbox.min.y, bbox.min.z),
        P3::new(bbox.max.x, bbox.min.y, bbox.min.z),
        P3::new(bbox.min.x, bbox.max.y, bbox.min.z),
        P3::new(bbox.max.x, bbox.max.y, bbox.min.z),
        P3::new(bbox.min.x, bbox.min.y, bbox.max.z),
        P3::new(bbox.max.x, bbox.min.y, bbox.max.z),
        P3::new(bbox.min.x, bbox.max.y, bbox.max.z),
        P3::new(bbox.max.x, bbox.max.y, bbox.max.z),
    ];
    let mut min = P3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = P3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for c in corners {
        let p = info.world_to_local(c);
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        min.z = min.z.min(p.z);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
        max.z = max.z.max(p.z);
    }
    BoundingBox3 { min, max }
}

/// Build a [`HeightContext`] for a session toolpath config using session data.
///
/// F-030: delegates to [`rs_cam_core::session::SetupEvalContext`] so the
/// Heights diagnostic, the generation path, and the simulator's depth
/// reference all read from one source of truth. `stock_top_z` /
/// `stock_bottom_z` come from `ctx.heights_stock_bbox` (world frame for
/// identity setups, local zero-rooted for non-identity); `safe_z` is the
/// F-024 floor; model Z extents are reported in the setup-local frame for
/// non-identity setups so the Heights tab references match the frame the
/// toolpath is actually computed in.
pub fn height_context_from_session(
    session: &rs_cam_core::session::ProjectSession,
    tc: &rs_cam_core::session::ToolpathConfig,
) -> rs_cam_core::compute::config::HeightContext {
    let setup = session.list_setups().iter().find(|s| {
        s.toolpath_indices.iter().any(|&i| {
            session
                .toolpath_configs()
                .get(i)
                .is_some_and(|t| t.id == tc.id)
        })
    });
    let ctx = rs_cam_core::session::SetupEvalContext::build_for_setup(session, setup);
    let raw_mb = session
        .models()
        .iter()
        .find(|m| m.id == tc.model_id)
        .and_then(|m| {
            m.mesh.as_ref().map(|mesh| mesh.bbox).or_else(|| {
                m.polygons
                    .as_deref()
                    .and_then(|polys| rs_cam_core::session::polygons_bbox(polys))
            })
        });
    // For non-identity setups, project the raw world-frame model bbox into
    // the setup-local frame so the Heights tab numbers match the toolpath
    // generator's frame.
    let mb = match (raw_mb, ctx.local_to_global.as_ref()) {
        (Some(b), Some(info)) => Some(setup_local_bbox(&b, info)),
        (Some(b), None) => Some(b),
        _ => None,
    };
    let heights_bbox = ctx.heights_stock_bbox;
    rs_cam_core::compute::config::HeightContext {
        safe_z: ctx.safe_z,
        op_depth: tc.operation.default_depth_for_heights(),
        stock_top_z: heights_bbox.max.z,
        stock_bottom_z: heights_bbox.min.z,
        model_top_z: mb.map(|b| b.max.z),
        model_bottom_z: mb.map(|b| b.min.z),
    }
}

/// Bounding box of a session `Fixture` (physical extents, no clearance).
pub fn session_fixture_bbox(f: &rs_cam_core::session::Fixture) -> BoundingBox3 {
    use rs_cam_core::geo::P3;
    BoundingBox3 {
        min: P3::new(f.origin_x, f.origin_y, f.origin_z),
        max: P3::new(
            f.origin_x + f.size_x,
            f.origin_y + f.size_y,
            f.origin_z + f.size_z,
        ),
    }
}

/// Clearance bounding box of a session `Fixture` (inflated by clearance margin).
pub fn session_fixture_clearance_bbox(f: &rs_cam_core::session::Fixture) -> BoundingBox3 {
    use rs_cam_core::geo::P3;
    let c = f.clearance;
    BoundingBox3 {
        min: P3::new(f.origin_x - c, f.origin_y - c, f.origin_z),
        max: P3::new(
            f.origin_x + f.size_x + c,
            f.origin_y + f.size_y + c,
            f.origin_z + f.size_z,
        ),
    }
}

/// Bounding box of a session `KeepOutZone` (full Z extent of stock).
pub fn session_keep_out_bbox(
    ko: &rs_cam_core::session::KeepOutZone,
    stock: &StockConfig,
) -> BoundingBox3 {
    use rs_cam_core::geo::P3;
    BoundingBox3 {
        min: P3::new(ko.origin_x, ko.origin_y, stock.origin_z),
        max: P3::new(
            ko.origin_x + ko.size_x,
            ko.origin_y + ko.size_y,
            stock.origin_z + stock.z,
        ),
    }
}

/// Transform a mesh into a setup's local coordinate frame.
///
/// Thin delegate — the algorithm lives in
/// [`rs_cam_core::compute::transform::SetupTransformInfo::apply_to_mesh`].
pub fn transform_mesh(
    mesh: &rs_cam_core::mesh::TriangleMesh,
    setup: &SetupFrame,
    stock: &StockConfig,
) -> rs_cam_core::mesh::TriangleMesh {
    setup.transform_info(stock).apply_to_mesh(mesh)
}

/// Transform a StockMesh's vertices from global frame to a setup's local frame.
/// Modifies the mesh in place — vertices are stored as flat [x, y, z, ...] f32.
#[allow(clippy::indexing_slicing)] // stride-3 loop bounded by mesh.vertices.len()
pub fn transform_heightmap_mesh(
    mesh: &mut rs_cam_core::simulation::StockMesh,
    setup: &SetupFrame,
    stock: &StockConfig,
) {
    for i in (0..mesh.vertices.len()).step_by(3) {
        let p = rs_cam_core::geo::P3::new(
            mesh.vertices[i] as f64,
            mesh.vertices[i + 1] as f64,
            mesh.vertices[i + 2] as f64,
        );
        let local = setup.transform_point(p, stock);
        mesh.vertices[i] = local.x as f32;
        mesh.vertices[i + 1] = local.y as f32;
        mesh.vertices[i + 2] = local.z as f32;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use rs_cam_core::geo::P3;

    fn stock_at_origin() -> StockConfig {
        StockConfig {
            x: 100.0,
            y: 80.0,
            z: 25.0,
            ..StockConfig::default()
        }
    }

    fn stock_with_offset() -> StockConfig {
        StockConfig {
            x: 100.0,
            y: 80.0,
            z: 25.0,
            origin_x: -50.0,
            origin_y: -40.0,
            origin_z: 0.0,
            ..StockConfig::default()
        }
    }

    /// Identity setups emit in world frame: the display shift must move
    /// them by exactly -origin so they land on the origin-subtracted
    /// mesh/stock the viewport draws.
    #[test]
    fn emission_shift_identity_is_negated_origin() {
        let stock = StockConfig {
            x: 200.0,
            y: 171.5,
            z: 25.0,
            origin_x: 3.0,
            origin_y: -4.0,
            origin_z: -19.0,
            ..StockConfig::default()
        };
        let setup = SetupFrame::new(FaceUp::Top, ZRotation::Deg0);
        let shift = setup.emission_to_display_shift(&stock);
        assert_eq!((shift.x, shift.y, shift.z), (-3.0, 4.0, 19.0));
    }

    /// Non-identity setups already emit in the local display frame —
    /// shifting them again would double-transform.
    #[test]
    fn emission_shift_non_identity_is_zero() {
        let stock = stock_with_offset();
        for setup in [
            SetupFrame::new(FaceUp::Bottom, ZRotation::Deg0),
            SetupFrame::new(FaceUp::Top, ZRotation::Deg90),
        ] {
            let shift = setup.emission_to_display_shift(&stock);
            assert_eq!((shift.x, shift.y, shift.z), (0.0, 0.0, 0.0));
        }
    }

    /// Verify that transform followed by inverse_transform is identity for all orientations.
    #[test]
    fn setup_transform_round_trip() {
        // Test both zero-origin and non-zero origin stocks
        for stock in [stock_at_origin(), stock_with_offset()] {
            let point = P3::new(
                stock.origin_x + 30.0,
                stock.origin_y + 20.0,
                stock.origin_z + 10.0,
            );

            for &face in FaceUp::ALL {
                for &rot in ZRotation::ALL {
                    let setup = SetupFrame::new(face, rot);

                    let transformed = setup.transform_point(point, &stock);
                    let recovered = setup.inverse_transform_point(transformed, &stock);

                    assert!(
                        (recovered.x - point.x).abs() < 1e-10
                            && (recovered.y - point.y).abs() < 1e-10
                            && (recovered.z - point.z).abs() < 1e-10,
                        "Round-trip failed for face={:?} rot={:?} origin=({},{},{}): {:?} -> {:?} -> {:?}",
                        face,
                        rot,
                        stock.origin_x,
                        stock.origin_y,
                        stock.origin_z,
                        point,
                        transformed,
                        recovered,
                    );
                }
            }
        }
    }

    /// Round-trip with multiple different test points per combination,
    /// including corners and center of stock volume.
    #[test]
    fn setup_transform_round_trip_multiple_points() {
        let stock = stock_at_origin();

        let test_points = [
            // Interior point
            P3::new(30.0, 20.0, 10.0),
            // Origin corner
            P3::new(0.0, 0.0, 0.0),
            // Far corner
            P3::new(stock.x, stock.y, stock.z),
            // Center of stock
            P3::new(stock.x / 2.0, stock.y / 2.0, stock.z / 2.0),
            // Edge midpoints
            P3::new(stock.x / 2.0, 0.0, stock.z / 2.0),
            P3::new(0.0, stock.y / 2.0, stock.z / 2.0),
        ];

        for &face in FaceUp::ALL {
            for &rot in ZRotation::ALL {
                let setup = SetupFrame::new(face, rot);

                for &point in &test_points {
                    let transformed = setup.transform_point(point, &stock);
                    let recovered = setup.inverse_transform_point(transformed, &stock);

                    assert!(
                        (recovered.x - point.x).abs() < 1e-10
                            && (recovered.y - point.y).abs() < 1e-10
                            && (recovered.z - point.z).abs() < 1e-10,
                        "Round-trip failed for face={:?} rot={:?} point={:?}: got {:?}",
                        face,
                        rot,
                        point,
                        recovered,
                    );
                }
            }
        }
    }

    /// Verify specific known transforms produce expected results.
    #[test]
    fn face_up_bottom_flips_z() {
        let stock = stock_at_origin();
        let setup = SetupFrame::new(FaceUp::Bottom, ZRotation::Deg0);

        // A point at the top of the stock (z = stock.z) should map to z = 0
        let top_point = P3::new(50.0, 40.0, stock.z);
        let transformed = setup.transform_point(top_point, &stock);
        assert!(
            transformed.z.abs() < 1e-10,
            "FaceUp::Bottom should map stock top (z={}) to z=0, got z={}",
            stock.z,
            transformed.z
        );

        // A point at z = 0 should map to z = stock.z
        let bottom_point = P3::new(50.0, 40.0, 0.0);
        let transformed = setup.transform_point(bottom_point, &stock);
        assert!(
            (transformed.z - stock.z).abs() < 1e-10,
            "FaceUp::Bottom should map z=0 to z={}, got z={}",
            stock.z,
            transformed.z
        );
    }

    /// Verify FaceUp::Top with Deg0 is identity (no transform).
    #[test]
    fn identity_setup_is_passthrough() {
        let stock = stock_at_origin();
        let setup = SetupFrame::default();

        let point = P3::new(30.0, 20.0, 10.0);
        let transformed = setup.transform_point(point, &stock);

        assert!(
            (transformed.x - point.x).abs() < 1e-10
                && (transformed.y - point.y).abs() < 1e-10
                && (transformed.z - point.z).abs() < 1e-10,
            "Identity setup should be passthrough: {:?} -> {:?}",
            point,
            transformed
        );
    }

    /// Verify ZRotation::Deg90 swaps X and Y dimensions.
    #[test]
    fn z_rotation_90_swaps_axes() {
        let stock = stock_at_origin();
        let setup = SetupFrame::new(FaceUp::Top, ZRotation::Deg90);

        // The origin (0,0,z) should map to (D, 0, z) under 90 deg rotation
        // since Deg90 formula: new_x = D - y, new_y = x
        let point = P3::new(0.0, 0.0, 10.0);
        let transformed = setup.transform_point(point, &stock);

        assert!(
            (transformed.x - stock.y).abs() < 1e-10,
            "Deg90: origin.x should map to stock.y={}, got {}",
            stock.y,
            transformed.x
        );
        assert!(
            transformed.y.abs() < 1e-10,
            "Deg90: origin.y should map to 0, got {}",
            transformed.y
        );
        assert!(
            (transformed.z - 10.0).abs() < 1e-10,
            "Deg90: z should be preserved, got {}",
            transformed.z
        );
    }

    /// Verify FaceUp::Front rotates Y and Z.
    ///
    /// Updated for **G-FRONTNAME** (2026-08-22). This test transcribed the
    /// old arm `(x, H-z, y)`, whose local +Z is world +Y — i.e. it brought
    /// the **+Y** face up, which drafting calls the *back*. The operator
    /// ruled that the drafting convention wins, so `Front` now means the −Y
    /// face and the arm is `(x, z, D-y)`. The world-face meaning itself is
    /// pinned in core by
    /// `tests/face_up_names_follow_drafting_convention_g_frontname.rs`; this
    /// one stays as the viz-side check that `SetupFrame::transform_point`
    /// delegates to it rather than growing its own copy of the arithmetic.
    #[test]
    fn face_up_front_rotates_y_z() {
        let stock = stock_at_origin();
        let setup = SetupFrame::new(FaceUp::Front, ZRotation::Deg0);

        // Front: new = (x, z, D-y) where D = stock.y
        let point = P3::new(30.0, 20.0, 10.0);
        let transformed = setup.transform_point(point, &stock);

        assert!(
            (transformed.x - 30.0).abs() < 1e-10,
            "Front: x should be preserved"
        );
        assert!(
            (transformed.y - 10.0).abs() < 1e-10,
            "Front: new_y should be old_z = 10, got {}",
            transformed.y
        );
        assert!(
            (transformed.z - (stock.y - 20.0)).abs() < 1e-10,
            "Front: new_z should be D - old_y = {}, got {}",
            stock.y - 20.0,
            transformed.z
        );
    }

    /// Transformed coordinates should stay non-negative within stock bounds.
    #[test]
    fn transformed_coords_stay_non_negative_for_interior_points() {
        let stock = stock_at_origin();

        for &face in FaceUp::ALL {
            for &rot in ZRotation::ALL {
                let setup = SetupFrame::new(face, rot);

                // A point in the interior of the stock
                let point = P3::new(stock.x * 0.3, stock.y * 0.3, stock.z * 0.3);
                let transformed = setup.transform_point(point, &stock);

                assert!(
                    transformed.x >= -1e-10 && transformed.y >= -1e-10 && transformed.z >= -1e-10,
                    "Interior point should transform to non-negative coords for face={:?} rot={:?}: got {:?}",
                    face,
                    rot,
                    transformed
                );
            }
        }
    }

    #[test]
    fn all_constants_are_exhaustive() {
        // ToolType: 5 variants
        assert_eq!(
            ToolType::ALL.len(),
            5,
            "ToolType::ALL out of sync with enum"
        );
        // PostFormat: 4 variants (Grbl, GrblHal, LinuxCnc, Mach3)
        assert_eq!(
            PostFormat::ALL.len(),
            4,
            "PostFormat::ALL out of sync with enum"
        );
        // ToolMaterial: 2 variants
        assert_eq!(
            ToolMaterial::ALL.len(),
            2,
            "ToolMaterial::ALL out of sync with enum"
        );
        // BitCutDirection: 3 variants
        assert_eq!(
            BitCutDirection::ALL.len(),
            3,
            "BitCutDirection::ALL out of sync with enum"
        );
        // FaceUp: 6 variants
        assert_eq!(FaceUp::ALL.len(), 6, "FaceUp::ALL out of sync with enum");
        // ZRotation: 4 variants
        assert_eq!(
            ZRotation::ALL.len(),
            4,
            "ZRotation::ALL out of sync with enum"
        );
        // Corner: 4 variants
        assert_eq!(Corner::ALL.len(), 4, "Corner::ALL out of sync with enum");
    }

    #[test]
    fn all_constants_have_no_duplicates() {
        use std::collections::HashSet;
        let tool_types: HashSet<_> = ToolType::ALL.iter().collect();
        assert_eq!(
            tool_types.len(),
            ToolType::ALL.len(),
            "ToolType::ALL has duplicates"
        );
        let post_formats: HashSet<_> = PostFormat::ALL.iter().collect();
        assert_eq!(
            post_formats.len(),
            PostFormat::ALL.len(),
            "PostFormat::ALL has duplicates"
        );
        let face_ups: HashSet<_> = FaceUp::ALL.iter().collect();
        assert_eq!(
            face_ups.len(),
            FaceUp::ALL.len(),
            "FaceUp::ALL has duplicates"
        );
        let z_rots: HashSet<_> = ZRotation::ALL.iter().collect();
        assert_eq!(
            z_rots.len(),
            ZRotation::ALL.len(),
            "ZRotation::ALL has duplicates"
        );
        let corners: HashSet<_> = Corner::ALL.iter().collect();
        assert_eq!(
            corners.len(),
            Corner::ALL.len(),
            "Corner::ALL has duplicates"
        );
    }
}
