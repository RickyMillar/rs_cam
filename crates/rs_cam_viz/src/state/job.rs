use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::enriched_mesh::EnrichedMesh;
use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::Polygon2;
// Serialize/Deserialize not directly needed in this file any more
// (GUI-local types like Corner don't derive Serialize)

// ── Re-exports from rs_cam_core::compute (Phase 1 service layer extraction) ──
pub use rs_cam_core::compute::stock_config::{
    AlignmentPin, FixtureId, FlipAxis, KeepOutId, ModelId, ModelKind, ModelUnits, PostConfig,
    PostFormat, SetupId, StockConfig,
};
pub use rs_cam_core::compute::tool_config::{
    BitCutDirection, ToolConfig, ToolId, ToolMaterial, ToolType,
};
pub use rs_cam_core::compute::transform::{FaceUp, ZRotation};

/// A loaded geometry model.
pub struct LoadedModel {
    pub id: ModelId,
    pub path: PathBuf,
    pub name: String,
    pub kind: ModelKind,
    pub mesh: Option<Arc<TriangleMesh>>,
    pub polygons: Option<Arc<Vec<Polygon2>>>,
    pub enriched_mesh: Option<Arc<EnrichedMesh>>,
    pub units: ModelUnits,
    /// Percentage of inconsistent winding edges (from check_winding). None if not STL.
    pub winding_report: Option<f64>,
    /// Load/import failure preserved so broken references can round-trip.
    pub load_error: Option<String>,
}

impl LoadedModel {
    pub fn placeholder(
        id: ModelId,
        path: PathBuf,
        name: String,
        kind: ModelKind,
        units: ModelUnits,
        load_error: String,
    ) -> Self {
        Self {
            id,
            path,
            name,
            kind,
            mesh: None,
            polygons: None,
            enriched_mesh: None,
            units,
            winding_report: None,
            load_error: Some(load_error),
        }
    }

    pub fn bbox(&self) -> Option<BoundingBox3> {
        if let Some(mesh) = &self.mesh {
            return Some(mesh.bbox);
        }

        let polygons = self.polygons.as_deref()?;
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for polygon in polygons {
            for point in polygon
                .exterior
                .iter()
                .chain(polygon.holes.iter().flat_map(|hole| hole.iter()))
            {
                min_x = min_x.min(point.x);
                min_y = min_y.min(point.y);
                max_x = max_x.max(point.x);
                max_y = max_y.max(point.y);
            }
        }

        if !min_x.is_finite() {
            return None;
        }

        Some(BoundingBox3 {
            min: rs_cam_core::geo::P3::new(min_x, min_y, 0.0),
            max: rs_cam_core::geo::P3::new(max_x, max_y, 0.0),
        })
    }
}

// ToolConfig, ToolType, ToolMaterial, BitCutDirection, PostConfig, PostFormat,
// StockConfig, AlignmentPin, FlipAxis, ModelId, ToolId, SetupId, FixtureId,
// KeepOutId, ModelKind, ModelUnits are now re-exported from core above.

// FaceUp and ZRotation are now re-exported from core::compute::transform above.

/// Which corner of the stock to probe for XY datum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Corner {
    FrontLeft,
    FrontRight,
    BackLeft,
    BackRight,
}

impl Corner {
    pub const ALL: &[Corner] = &[
        Corner::FrontLeft,
        Corner::FrontRight,
        Corner::BackLeft,
        Corner::BackRight,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Corner::FrontLeft => "Front-Left",
            Corner::FrontRight => "Front-Right",
            Corner::BackLeft => "Back-Left",
            Corner::BackRight => "Back-Right",
        }
    }

    pub fn to_key(&self) -> &'static str {
        match self {
            Corner::FrontLeft => "fl",
            Corner::FrontRight => "fr",
            Corner::BackLeft => "bl",
            Corner::BackRight => "br",
        }
    }

    pub fn from_key(s: &str) -> Self {
        match s {
            "fr" => Corner::FrontRight,
            "bl" => Corner::BackLeft,
            "br" => Corner::BackRight,
            _ => Corner::FrontLeft,
        }
    }
}

/// How the operator establishes XY zero for this setup.
#[derive(Debug, Clone, PartialEq)]
pub enum XYDatum {
    CornerProbe(Corner),
    CenterOfStock,
    AlignmentPins,
    Manual,
}

impl Default for XYDatum {
    fn default() -> Self {
        XYDatum::CornerProbe(Corner::FrontLeft)
    }
}

impl XYDatum {
    pub fn label(&self) -> &str {
        match self {
            XYDatum::CornerProbe(c) => match c {
                Corner::FrontLeft => "Corner Probe (Front-Left)",
                Corner::FrontRight => "Corner Probe (Front-Right)",
                Corner::BackLeft => "Corner Probe (Back-Left)",
                Corner::BackRight => "Corner Probe (Back-Right)",
            },
            XYDatum::CenterOfStock => "Center of Stock",
            XYDatum::AlignmentPins => "Alignment Pins",
            XYDatum::Manual => "Manual",
        }
    }

    pub fn to_key(&self) -> String {
        match self {
            XYDatum::CornerProbe(c) => format!("corner_{}", c.to_key()),
            XYDatum::CenterOfStock => "center".into(),
            XYDatum::AlignmentPins => "pins".into(),
            XYDatum::Manual => "manual".into(),
        }
    }

    pub fn from_key(s: &str) -> Self {
        if let Some(corner) = s.strip_prefix("corner_") {
            XYDatum::CornerProbe(Corner::from_key(corner))
        } else {
            match s {
                "center" => XYDatum::CenterOfStock,
                "pins" => XYDatum::AlignmentPins,
                "manual" => XYDatum::Manual,
                _ => XYDatum::default(),
            }
        }
    }
}

/// How the operator establishes Z zero for this setup.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ZDatum {
    #[default]
    StockTop,
    MachineTable,
    FixedOffset(f64),
    Manual,
}

impl ZDatum {
    pub fn label(&self) -> String {
        match self {
            ZDatum::StockTop => "Stock Top".into(),
            ZDatum::MachineTable => "Machine Table".into(),
            ZDatum::FixedOffset(z) => format!("Fixed Offset ({z:.1} mm)"),
            ZDatum::Manual => "Manual".into(),
        }
    }

    pub fn to_key(&self) -> String {
        match self {
            ZDatum::StockTop => "stock_top".into(),
            ZDatum::MachineTable => "table".into(),
            ZDatum::FixedOffset(z) => format!("offset:{z}"),
            ZDatum::Manual => "manual".into(),
        }
    }

    pub fn from_key(s: &str) -> Self {
        if let Some(val) = s.strip_prefix("offset:") {
            ZDatum::FixedOffset(val.parse().unwrap_or(0.0))
        } else {
            match s {
                "table" => ZDatum::MachineTable,
                "manual" => ZDatum::Manual,
                _ => ZDatum::StockTop,
            }
        }
    }
}

/// How to establish the work coordinate system for a setup.
#[derive(Debug, Clone, Default)]
pub struct DatumConfig {
    pub xy_method: XYDatum,
    pub z_method: ZDatum,
    pub notes: String,
}

// FlipAxis and AlignmentPin are now re-exported from core::compute::stock_config above.

/// Kind of workholding fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureKind {
    Clamp,
    Vise,
    VacuumPod,
    Custom,
}

impl FixtureKind {
    pub const ALL: &[FixtureKind] = &[
        FixtureKind::Clamp,
        FixtureKind::Vise,
        FixtureKind::VacuumPod,
        FixtureKind::Custom,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            FixtureKind::Clamp => "Clamp",
            FixtureKind::Vise => "Vise",
            FixtureKind::VacuumPod => "Vacuum Pod",
            FixtureKind::Custom => "Custom",
        }
    }
}

/// A physical workholding device positioned on the machine table.
#[derive(Debug, Clone)]
pub struct Fixture {
    pub id: FixtureId,
    pub name: String,
    pub kind: FixtureKind,
    pub enabled: bool,
    /// Position of the fixture's min corner in workpiece coordinates (mm).
    pub origin_x: f64,
    pub origin_y: f64,
    pub origin_z: f64,
    /// Dimensions of the fixture bounding box (mm).
    pub size_x: f64,
    pub size_y: f64,
    pub size_z: f64,
    /// Extra clearance around the fixture for tool avoidance (mm).
    pub clearance: f64,
}

impl Fixture {
    pub fn new_default(id: FixtureId) -> Self {
        Self {
            id,
            name: format!("Fixture {}", id.0 + 1),
            kind: FixtureKind::Clamp,
            enabled: true,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            size_x: 30.0,
            size_y: 15.0,
            size_z: 20.0,
            clearance: 3.0,
        }
    }

    /// Physical bounding box of the fixture.
    pub fn bbox(&self) -> BoundingBox3 {
        use rs_cam_core::geo::P3;
        BoundingBox3 {
            min: P3::new(self.origin_x, self.origin_y, self.origin_z),
            max: P3::new(
                self.origin_x + self.size_x,
                self.origin_y + self.size_y,
                self.origin_z + self.size_z,
            ),
        }
    }

    /// Bounding box inflated by the clearance margin (used for avoidance).
    pub fn clearance_bbox(&self) -> BoundingBox3 {
        use rs_cam_core::geo::P3;
        let c = self.clearance;
        BoundingBox3 {
            min: P3::new(self.origin_x - c, self.origin_y - c, self.origin_z),
            max: P3::new(
                self.origin_x + self.size_x + c,
                self.origin_y + self.size_y + c,
                self.origin_z + self.size_z,
            ),
        }
    }

    /// XY footprint (clearance bbox projected) as a polygon for boundary subtraction.
    pub fn footprint(&self) -> rs_cam_core::polygon::Polygon2 {
        let bb = self.clearance_bbox();
        rs_cam_core::polygon::Polygon2::rectangle(bb.min.x, bb.min.y, bb.max.x, bb.max.y)
    }
}

/// A rectangular region the tool must avoid (XY only, full Z extent).
#[derive(Debug, Clone)]
pub struct KeepOutZone {
    pub id: KeepOutId,
    pub name: String,
    pub enabled: bool,
    /// Position of the zone's min corner (mm).
    pub origin_x: f64,
    pub origin_y: f64,
    /// Dimensions of the zone (mm).
    pub size_x: f64,
    pub size_y: f64,
}

impl KeepOutZone {
    pub fn new_default(id: KeepOutId) -> Self {
        Self {
            id,
            name: format!("Keep-Out {}", id.0 + 1),
            enabled: true,
            origin_x: 0.0,
            origin_y: 0.0,
            size_x: 20.0,
            size_y: 20.0,
        }
    }

    /// 3D bounding box, extending from the stock Z range.
    pub fn bbox(&self, stock: &StockConfig) -> rs_cam_core::geo::BoundingBox3 {
        rs_cam_core::geo::BoundingBox3 {
            min: rs_cam_core::geo::P3::new(self.origin_x, self.origin_y, stock.origin_z),
            max: rs_cam_core::geo::P3::new(
                self.origin_x + self.size_x,
                self.origin_y + self.size_y,
                stock.origin_z + stock.z,
            ),
        }
    }

    /// XY footprint as a polygon for boundary subtraction.
    pub fn footprint(&self) -> rs_cam_core::polygon::Polygon2 {
        rs_cam_core::polygon::Polygon2::rectangle(
            self.origin_x,
            self.origin_y,
            self.origin_x + self.size_x,
            self.origin_y + self.size_y,
        )
    }
}

/// A named group of toolpaths sharing a common workholding context.
pub struct Setup {
    pub id: SetupId,
    pub name: String,
    pub face_up: FaceUp,
    pub z_rotation: ZRotation,
    pub datum: DatumConfig,
    pub fixtures: Vec<Fixture>,
    pub keep_out_zones: Vec<KeepOutZone>,
    pub toolpaths: Vec<super::toolpath::ToolpathEntry>,
}

impl Setup {
    pub fn new(id: SetupId, name: String) -> Self {
        Self {
            id,
            name,
            face_up: FaceUp::default(),
            z_rotation: ZRotation::default(),
            datum: DatumConfig::default(),
            fixtures: Vec::new(),
            keep_out_zones: Vec::new(),
            toolpaths: Vec::new(),
        }
    }

    /// Transform a point from world coords to this setup's local frame.
    /// Translates to stock-relative coords first, then applies FaceUp + ZRotation.
    pub fn transform_point(
        &self,
        p: rs_cam_core::geo::P3,
        stock: &StockConfig,
    ) -> rs_cam_core::geo::P3 {
        use rs_cam_core::geo::P3;
        // 1. Translate world → stock-relative (origin at 0,0,0)
        let rel = P3::new(
            p.x - stock.origin_x,
            p.y - stock.origin_y,
            p.z - stock.origin_z,
        );
        // 2. Apply FaceUp flip on stock-relative coords
        let flipped = self.face_up.transform_point(rel, stock.x, stock.y, stock.z);
        // 3. Apply ZRotation
        let (eff_w, eff_d, _) = self.face_up.effective_stock(stock.x, stock.y, stock.z);
        self.z_rotation.transform_point(flipped, eff_w, eff_d)
    }

    /// Effective stock dimensions in this setup's local frame.
    pub fn effective_stock(&self, stock: &StockConfig) -> (f64, f64, f64) {
        let (w, d, h) = self.face_up.effective_stock(stock.x, stock.y, stock.z);
        self.z_rotation.effective_stock(w, d, h)
    }

    /// Inverse transform: from this setup's local frame back to world coords.
    /// Undoes ZRotation, then FaceUp, then translates back to world coords.
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

/// Transform a mesh into a setup's local coordinate frame.
pub fn transform_mesh(
    mesh: &rs_cam_core::mesh::TriangleMesh,
    setup: &Setup,
    stock: &StockConfig,
) -> rs_cam_core::mesh::TriangleMesh {
    let new_verts: Vec<rs_cam_core::geo::P3> = mesh
        .vertices
        .iter()
        .map(|v| setup.transform_point(*v, stock))
        .collect();
    rs_cam_core::mesh::TriangleMesh::from_raw(new_verts, mesh.triangles.clone())
}

/// Transform a StockMesh's vertices from global frame to a setup's local frame.
/// Modifies the mesh in place — vertices are stored as flat [x, y, z, ...] f32.
#[allow(clippy::indexing_slicing)] // stride-3 loop bounded by mesh.vertices.len()
pub fn transform_heightmap_mesh(
    mesh: &mut rs_cam_core::simulation::StockMesh,
    setup: &Setup,
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

/// Transform 2D polygons into a setup's local frame (XY projection).
pub fn transform_polygons(
    polygons: &[rs_cam_core::polygon::Polygon2],
    setup: &Setup,
    stock: &StockConfig,
) -> Vec<rs_cam_core::polygon::Polygon2> {
    use rs_cam_core::geo::{P2, P3};

    polygons
        .iter()
        .map(|poly| {
            let ext: Vec<P2> = poly
                .exterior
                .iter()
                .map(|p| {
                    let p3 = setup.transform_point(P3::new(p.x, p.y, 0.0), stock);
                    P2::new(p3.x, p3.y)
                })
                .collect();
            let holes: Vec<Vec<P2>> = poly
                .holes
                .iter()
                .map(|hole| {
                    hole.iter()
                        .map(|p| {
                            let p3 = setup.transform_point(P3::new(p.x, p.y, 0.0), stock);
                            P2::new(p3.x, p3.y)
                        })
                        .collect()
                })
                .collect();
            let mut result = rs_cam_core::polygon::Polygon2::with_holes(ext, holes);
            result.ensure_winding();
            result
        })
        .collect()
}

/// The full job state.
pub struct JobState {
    pub name: String,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub models: Vec<LoadedModel>,
    pub stock: StockConfig,
    pub tools: Vec<ToolConfig>,
    pub post: PostConfig,
    pub machine: rs_cam_core::machine::MachineProfile,
    pub setups: Vec<Setup>,
    /// Monotonic counter incremented on every edit (for staleness detection).
    pub edit_counter: u64,
    next_model_id: usize,
    next_tool_id: usize,
    next_toolpath_id: usize,
    next_setup_id: usize,
    next_fixture_id: usize,
    next_keep_out_id: usize,
}

impl JobState {
    pub fn new() -> Self {
        Self {
            name: "Untitled".to_owned(),
            file_path: None,
            dirty: false,
            models: Vec::new(),
            stock: StockConfig::default(),
            tools: Vec::new(),
            post: PostConfig::default(),
            machine: rs_cam_core::machine::MachineProfile::default(),
            setups: vec![Setup::new(SetupId(0), "Setup 1".into())],
            edit_counter: 0,
            next_model_id: 0,
            next_tool_id: 0,
            next_toolpath_id: 0,
            next_setup_id: 1,
            next_fixture_id: 0,
            next_keep_out_id: 0,
        }
    }

    pub fn next_model_id(&mut self) -> ModelId {
        let id = ModelId(self.next_model_id);
        self.next_model_id += 1;
        id
    }

    pub fn next_tool_id(&mut self) -> ToolId {
        let id = ToolId(self.next_tool_id);
        self.next_tool_id += 1;
        id
    }

    pub fn next_toolpath_id(&mut self) -> super::toolpath::ToolpathId {
        let id = super::toolpath::ToolpathId(self.next_toolpath_id);
        self.next_toolpath_id += 1;
        id
    }

    pub fn next_setup_id(&mut self) -> SetupId {
        let id = SetupId(self.next_setup_id);
        self.next_setup_id += 1;
        id
    }

    pub fn next_fixture_id(&mut self) -> FixtureId {
        let id = FixtureId(self.next_fixture_id);
        self.next_fixture_id += 1;
        id
    }

    pub fn next_keep_out_id(&mut self) -> KeepOutId {
        let id = KeepOutId(self.next_keep_out_id);
        self.next_keep_out_id += 1;
        id
    }

    /// Iterate over all toolpaths (flat view across all setups).
    pub fn all_toolpaths(&self) -> impl Iterator<Item = &super::toolpath::ToolpathEntry> {
        self.setups.iter().flat_map(|setup| setup.toolpaths.iter())
    }

    /// Mutable iteration over all toolpaths (flat view across all setups).
    pub fn all_toolpaths_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut super::toolpath::ToolpathEntry> {
        self.setups
            .iter_mut()
            .flat_map(|setup| setup.toolpaths.iter_mut())
    }

    /// Find a toolpath by ID across all setups.
    pub fn find_toolpath(
        &self,
        id: super::toolpath::ToolpathId,
    ) -> Option<&super::toolpath::ToolpathEntry> {
        self.all_toolpaths().find(|toolpath| toolpath.id == id)
    }

    /// Build a [`HeightContext`] for resolving a toolpath's heights from current stock/model state.
    pub fn height_context_for(
        &self,
        tp: &super::toolpath::ToolpathEntry,
    ) -> super::toolpath::HeightContext {
        let sb = self.stock.bbox();
        let mb = self
            .models
            .iter()
            .find(|m| m.id == tp.model_id)
            .and_then(|m| m.bbox());
        super::toolpath::HeightContext {
            safe_z: self.post.safe_z,
            op_depth: tp.operation.default_depth_for_heights(),
            stock_top_z: sb.max.z,
            stock_bottom_z: sb.min.z,
            model_top_z: mb.map(|b| b.max.z),
            model_bottom_z: mb.map(|b| b.min.z),
        }
    }

    /// Find a mutable toolpath by ID across all setups.
    pub fn find_toolpath_mut(
        &mut self,
        id: super::toolpath::ToolpathId,
    ) -> Option<&mut super::toolpath::ToolpathEntry> {
        self.all_toolpaths_mut().find(|toolpath| toolpath.id == id)
    }

    /// Total toolpath count across all setups.
    pub fn toolpath_count(&self) -> usize {
        self.setups.iter().map(|setup| setup.toolpaths.len()).sum()
    }

    /// Add a toolpath to the default (first) setup.
    pub fn push_toolpath(&mut self, entry: super::toolpath::ToolpathEntry) {
        if let Some(setup) = self.setups.first_mut() {
            setup.toolpaths.push(entry);
        }
    }

    /// Add a toolpath to a specific setup.
    pub fn push_toolpath_to_setup(
        &mut self,
        setup_id: SetupId,
        entry: super::toolpath::ToolpathEntry,
    ) {
        if let Some(setup) = self.setups.iter_mut().find(|setup| setup.id == setup_id) {
            setup.toolpaths.push(entry);
        }
    }

    /// Remove a toolpath by ID from whatever setup contains it.
    pub fn remove_toolpath(&mut self, id: super::toolpath::ToolpathId) {
        for setup in &mut self.setups {
            setup.toolpaths.retain(|toolpath| toolpath.id != id);
        }
    }

    /// Move a toolpath one position earlier within its setup. Returns true if moved.
    pub fn move_toolpath_up(&mut self, id: super::toolpath::ToolpathId) -> bool {
        for setup in &mut self.setups {
            if let Some(pos) = setup
                .toolpaths
                .iter()
                .position(|toolpath| toolpath.id == id)
            {
                if pos > 0 {
                    setup.toolpaths.swap(pos, pos - 1);
                    return true;
                }
                return false;
            }
        }
        false
    }

    /// Move a toolpath one position later within its setup. Returns true if moved.
    pub fn move_toolpath_down(&mut self, id: super::toolpath::ToolpathId) -> bool {
        for setup in &mut self.setups {
            if let Some(pos) = setup
                .toolpaths
                .iter()
                .position(|toolpath| toolpath.id == id)
            {
                if pos + 1 < setup.toolpaths.len() {
                    setup.toolpaths.swap(pos, pos + 1);
                    return true;
                }
                return false;
            }
        }
        false
    }

    /// Iterate toolpaths with a global index (for color assignment and stable ordering).
    pub fn toolpaths_enumerated(
        &self,
    ) -> impl Iterator<Item = (usize, &super::toolpath::ToolpathEntry)> {
        self.setups
            .iter()
            .flat_map(|setup| setup.toolpaths.iter())
            .enumerate()
    }

    /// Return the setup that owns a given toolpath ID.
    pub fn setup_of_toolpath(&self, id: super::toolpath::ToolpathId) -> Option<SetupId> {
        self.setups
            .iter()
            .find(|setup| setup.toolpaths.iter().any(|toolpath| toolpath.id == id))
            .map(|setup| setup.id)
    }

    /// Reorder a toolpath within its current setup to a target index. Returns true if moved.
    pub fn reorder_toolpath(&mut self, id: super::toolpath::ToolpathId, target_idx: usize) -> bool {
        for setup in &mut self.setups {
            if let Some(pos) = setup
                .toolpaths
                .iter()
                .position(|toolpath| toolpath.id == id)
            {
                let clamped = target_idx.min(setup.toolpaths.len().saturating_sub(1));
                if pos != clamped {
                    let entry = setup.toolpaths.remove(pos);
                    setup.toolpaths.insert(clamped, entry);
                    return true;
                }
                return false;
            }
        }
        false
    }

    /// Move a toolpath from its current setup to a target setup at a given index. Returns true if moved.
    pub fn move_toolpath_to_setup(
        &mut self,
        id: super::toolpath::ToolpathId,
        target_setup_id: SetupId,
        index: usize,
    ) -> bool {
        // Find and remove from source setup
        let mut entry = None;
        for setup in &mut self.setups {
            if let Some(pos) = setup
                .toolpaths
                .iter()
                .position(|toolpath| toolpath.id == id)
            {
                entry = Some(setup.toolpaths.remove(pos));
                break;
            }
        }
        let Some(entry) = entry else {
            return false;
        };

        // Insert into target setup
        if let Some(target) = self
            .setups
            .iter_mut()
            .find(|setup| setup.id == target_setup_id)
        {
            let clamped = index.min(target.toolpaths.len());
            target.toolpaths.insert(clamped, entry);
            true
        } else {
            false
        }
    }

    /// Mark the job as edited (increments edit counter for staleness tracking).
    pub fn mark_edited(&mut self) {
        self.dirty = true;
        self.edit_counter += 1;
    }

    pub fn sync_next_ids(&mut self) {
        self.next_model_id = self
            .models
            .iter()
            .map(|m| m.id.0)
            .max()
            .map_or(0, |id| id + 1);
        self.next_tool_id = self
            .tools
            .iter()
            .map(|t| t.id.0)
            .max()
            .map_or(0, |id| id + 1);
        self.next_toolpath_id = self
            .setups
            .iter()
            .flat_map(|setup| setup.toolpaths.iter())
            .map(|toolpath| toolpath.id.0)
            .max()
            .map_or(0, |id| id + 1);
        self.next_setup_id = self
            .setups
            .iter()
            .map(|setup| setup.id.0)
            .max()
            .map_or(0, |id| id + 1);
        self.next_fixture_id = self
            .setups
            .iter()
            .flat_map(|setup| setup.fixtures.iter())
            .map(|fixture| fixture.id.0)
            .max()
            .map_or(0, |id| id + 1);
        self.next_keep_out_id = self
            .setups
            .iter()
            .flat_map(|setup| setup.keep_out_zones.iter())
            .map(|keep_out| keep_out.id.0)
            .max()
            .map_or(0, |id| id + 1);
    }
}

impl Default for JobState {
    fn default() -> Self {
        Self::new()
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
                    let setup = Setup {
                        face_up: face,
                        z_rotation: rot,
                        ..Setup::new(SetupId(0), "Test".to_owned())
                    };

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
                let setup = Setup {
                    face_up: face,
                    z_rotation: rot,
                    ..Setup::new(SetupId(0), "Test".to_owned())
                };

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
        let setup = Setup {
            face_up: FaceUp::Bottom,
            z_rotation: ZRotation::Deg0,
            ..Setup::new(SetupId(0), "Test".to_owned())
        };

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
        let setup = Setup::new(SetupId(0), "Test".to_owned());

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
        let setup = Setup {
            face_up: FaceUp::Top,
            z_rotation: ZRotation::Deg90,
            ..Setup::new(SetupId(0), "Test".to_owned())
        };

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
    #[test]
    fn face_up_front_rotates_y_z() {
        let stock = stock_at_origin();
        let setup = Setup {
            face_up: FaceUp::Front,
            z_rotation: ZRotation::Deg0,
            ..Setup::new(SetupId(0), "Test".to_owned())
        };

        // Front: new = (x, H-z, y) where H = stock.z
        let point = P3::new(30.0, 20.0, 10.0);
        let transformed = setup.transform_point(point, &stock);

        assert!(
            (transformed.x - 30.0).abs() < 1e-10,
            "Front: x should be preserved"
        );
        assert!(
            (transformed.y - (stock.z - 10.0)).abs() < 1e-10,
            "Front: new_y should be H - old_z = {}, got {}",
            stock.z - 10.0,
            transformed.y
        );
        assert!(
            (transformed.z - 20.0).abs() < 1e-10,
            "Front: new_z should be old_y = 20, got {}",
            transformed.z
        );
    }

    /// Transformed coordinates should stay non-negative within stock bounds.
    #[test]
    fn transformed_coords_stay_non_negative_for_interior_points() {
        let stock = stock_at_origin();

        for &face in FaceUp::ALL {
            for &rot in ZRotation::ALL {
                let setup = Setup {
                    face_up: face,
                    z_rotation: rot,
                    ..Setup::new(SetupId(0), "Test".to_owned())
                };

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
        // PostFormat: 3 variants
        assert_eq!(
            PostFormat::ALL.len(),
            3,
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
        // FixtureKind: 4 variants
        assert_eq!(
            FixtureKind::ALL.len(),
            4,
            "FixtureKind::ALL out of sync with enum"
        );
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
