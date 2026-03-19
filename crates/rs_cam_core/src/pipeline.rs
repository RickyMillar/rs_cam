//! Incremental computation foundation — dirty-flag pipeline cache.
//!
//! Caches intermediate results (mesh, spatial index, surface heightmap)
//! across operations in a multi-operation job. When parameters change,
//! downstream stages are invalidated automatically.
//!
//! This is a foundation for the GUI phase — for now it enables the CLI
//! to skip recomputation in TOML multi-operation jobs where the mesh
//! and surface heightmap are shared across operations.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::slope::SurfaceHeightmap;

/// A cache key computed from parameters that affect a pipeline stage.
type StageKey = u64;

/// Compute a hash-based cache key from hashable parameters.
fn compute_key<T: Hash>(params: &T) -> StageKey {
    let mut hasher = DefaultHasher::new();
    params.hash(&mut hasher);
    hasher.finish()
}

/// Cached pipeline state for incremental recomputation.
///
/// Each stage stores its output alongside a key computed from its inputs.
/// When the key changes, the cached output is invalidated.
pub struct PipelineCache {
    /// Cached mesh + spatial index, keyed by (mesh_path, scale).
    mesh_key: Option<StageKey>,
    mesh: Option<(TriangleMesh, SpatialIndex)>,

    /// Cached surface heightmap, keyed by (mesh_key, cutter_desc, cell_size, bounds).
    surface_hm_key: Option<StageKey>,
    surface_hm: Option<SurfaceHeightmap>,
}

impl PipelineCache {
    pub fn new() -> Self {
        Self {
            mesh_key: None,
            mesh: None,
            surface_hm_key: None,
            surface_hm: None,
        }
    }

    /// Get or load a mesh. Returns a reference if the mesh matches the key,
    /// otherwise calls the loader function and caches the result.
    pub fn get_or_load_mesh<F>(
        &mut self,
        mesh_path: &str,
        scale: u64, // scaled to avoid f64 hashing issues
        loader: F,
    ) -> Option<&(TriangleMesh, SpatialIndex)>
    where
        F: FnOnce() -> Option<(TriangleMesh, SpatialIndex)>,
    {
        let key = compute_key(&(mesh_path, scale));

        if self.mesh_key == Some(key) {
            return self.mesh.as_ref();
        }

        // Invalidate downstream caches
        self.surface_hm_key = None;
        self.surface_hm = None;

        self.mesh = loader();
        if self.mesh.is_some() {
            self.mesh_key = Some(key);
        }
        self.mesh.as_ref()
    }

    /// Get or compute a surface heightmap. Invalidated when mesh changes.
    pub fn get_or_compute_surface_hm<F>(
        &mut self,
        cutter_desc: &str,
        cell_size_scaled: u64,
        compute: F,
    ) -> Option<&SurfaceHeightmap>
    where
        F: FnOnce(&TriangleMesh, &SpatialIndex) -> SurfaceHeightmap,
    {
        let mesh_key = self.mesh_key?;
        let key = compute_key(&(mesh_key, cutter_desc, cell_size_scaled));

        if self.surface_hm_key == Some(key) {
            return self.surface_hm.as_ref();
        }

        let (mesh, index) = self.mesh.as_ref()?;
        let hm = compute(mesh, index);
        self.surface_hm = Some(hm);
        self.surface_hm_key = Some(key);
        self.surface_hm.as_ref()
    }

    /// Invalidate all cached state.
    pub fn clear(&mut self) {
        self.mesh_key = None;
        self.mesh = None;
        self.surface_hm_key = None;
        self.surface_hm = None;
    }

    /// Check if a mesh is currently cached.
    pub fn has_mesh(&self) -> bool {
        self.mesh.is_some()
    }
}

impl Default for PipelineCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::make_test_flat;

    #[test]
    fn test_cache_hit() {
        let mut cache = PipelineCache::new();
        let mut load_count = 0;

        // First load
        cache.get_or_load_mesh("test.stl", 1000, || {
            load_count += 1;
            let mesh = make_test_flat(100.0);
            let index = SpatialIndex::build(&mesh, 20.0);
            Some((mesh, index))
        });
        assert_eq!(load_count, 1);
        assert!(cache.has_mesh());

        // Second load with same key — should be cached
        cache.get_or_load_mesh("test.stl", 1000, || {
            load_count += 1;
            None // Should not be called
        });
        assert_eq!(load_count, 1, "Second load should hit cache");
    }

    #[test]
    fn test_cache_miss_different_key() {
        let mut cache = PipelineCache::new();
        let mut load_count = 0;

        cache.get_or_load_mesh("test.stl", 1000, || {
            load_count += 1;
            let mesh = make_test_flat(100.0);
            let index = SpatialIndex::build(&mesh, 20.0);
            Some((mesh, index))
        });

        // Different scale — should reload
        cache.get_or_load_mesh("test.stl", 2000, || {
            load_count += 1;
            let mesh = make_test_flat(100.0);
            let index = SpatialIndex::build(&mesh, 20.0);
            Some((mesh, index))
        });
        assert_eq!(load_count, 2, "Different scale should reload");
    }

    #[test]
    fn test_downstream_invalidation() {
        let mut cache = PipelineCache::new();

        // Load mesh
        cache.get_or_load_mesh("test.stl", 1000, || {
            let mesh = make_test_flat(100.0);
            let index = SpatialIndex::build(&mesh, 20.0);
            Some((mesh, index))
        });

        // Compute surface heightmap
        let hm = cache.get_or_compute_surface_hm("ball:6", 100, |_mesh, _index| {
            // Minimal stub heightmap
            SurfaceHeightmap {
                z_values: vec![0.0; 100],
                rows: 10,
                cols: 10,
                origin_x: 0.0,
                origin_y: 0.0,
                cell_size: 1.0,
            }
        });
        assert!(hm.is_some());

        // Reload mesh with different key — should invalidate surface_hm
        cache.get_or_load_mesh("other.stl", 1000, || {
            let mesh = make_test_flat(50.0);
            let index = SpatialIndex::build(&mesh, 10.0);
            Some((mesh, index))
        });

        // surface_hm should be gone (check via computing again)
        let mut compute_count = 0;
        cache.get_or_compute_surface_hm("ball:6", 100, |_mesh, _index| {
            compute_count += 1;
            SurfaceHeightmap {
                z_values: vec![0.0; 100],
                rows: 10,
                cols: 10,
                origin_x: 0.0,
                origin_y: 0.0,
                cell_size: 1.0,
            }
        });
        assert_eq!(compute_count, 1, "Surface HM should be recomputed after mesh change");
    }

    #[test]
    fn test_clear() {
        let mut cache = PipelineCache::new();
        cache.get_or_load_mesh("test.stl", 1000, || {
            let mesh = make_test_flat(100.0);
            let index = SpatialIndex::build(&mesh, 20.0);
            Some((mesh, index))
        });
        assert!(cache.has_mesh());

        cache.clear();
        assert!(!cache.has_mesh());
    }
}
