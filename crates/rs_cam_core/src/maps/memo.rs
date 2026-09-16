//! The bounded, mesh-identity memo the map caches share.
//!
//! [`crate::maps::geom_cache`], [`crate::maps::tier_map_cache`] and
//! [`crate::maps::reach_map_cache`] each grew the same table: a `Vec` of entries
//! that hold a [`Weak<TriangleMesh>`] for identity, a lookup that upgrades the
//! `Weak` and compares with [`Arc::ptr_eq`], and an insert that sweeps
//! dead-mesh entries and then evicts oldest-first at `CAPACITY`. The argument
//! for that shape — why the identity key is a `Weak` and not a bare pointer
//! (the ABA hazard), and why identity implies content — is in `geom_cache`'s
//! module doc. This module is the one copy of the mechanism.
//!
//! Everything that is a decision stays with the cache that makes it: its
//! `CAPACITY`, its key type, its stats counters and its build function. The
//! memo owns no lock, so each cache keeps the lock scope its own doc
//! describes: the lock covers the lookup and the insert, never the build.
//!
//! [`crate::maps::finish_surface_cache`] is deliberately not a member of the TABLE.
//! It keys the mesh by CONTENT, because no call site on its path holds an
//! `Arc` to hang a `Weak` on (its module doc argues this at length), so it has
//! neither the `Weak` identity nor the dead-mesh sweep this memo is built
//! around. It does share [`CacheCounters`], which is independent of the table.
//!
//! [`CacheCounters`] is the second mechanism this module owns. All four map
//! caches counted builds and hits with a hand-written pair of `AtomicU64`
//! statics and a hand-written `stats()` / `reset_stats()` pair until
//! 2026-09-17. The STATICS stay with each cache, by the same rule as the
//! capacity and the key: `geom_cache` holds three of them, one per memoised
//! product, and each cache keeps its own public stats struct.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

use crate::mesh::TriangleMesh;

/// One cache's cumulative build and hit counters, since process start.
///
/// `Relaxed` throughout: the counters are a diagnostic, never a
/// synchronisation edge. A reader can see a build and its hit in either
/// order, which no consumer depends on.
#[derive(Debug, Default)]
pub struct CacheCounters {
    builds: AtomicU64,
    hits: AtomicU64,
}

impl CacheCounters {
    /// Zeroed counters, for a `static`.
    pub(crate) const fn new() -> Self {
        Self {
            builds: AtomicU64::new(0),
            hits: AtomicU64::new(0),
        }
    }

    /// Count one build.
    pub(crate) fn record_build(&self) {
        self.builds.fetch_add(1, Ordering::Relaxed);
    }

    /// Count one hit.
    pub(crate) fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Read the counters as `(builds, hits)`.
    pub(crate) fn read(&self) -> (u64, u64) {
        (
            self.builds.load(Ordering::Relaxed),
            self.hits.load(Ordering::Relaxed),
        )
    }

    /// Zero the counters. The cached values themselves are untouched, so this
    /// cannot change any result.
    pub(crate) fn reset(&self) {
        self.builds.store(0, Ordering::Relaxed);
        self.hits.store(0, Ordering::Relaxed);
    }
}

struct MemoEntry<K, V> {
    /// Liveness-checked identity key — see the module doc on why this is a
    /// `Weak` and not a raw pointer.
    mesh: Weak<TriangleMesh>,
    key: K,
    value: V,
}

impl<K: PartialEq, V> MemoEntry<K, V> {
    /// True when this entry is for `mesh` and `key` *and* `mesh` is the same
    /// live object it was created for.
    fn matches(&self, mesh: &Arc<TriangleMesh>, key: &K) -> bool {
        self.key == *key
            && self
                .mesh
                .upgrade()
                .is_some_and(|live| Arc::ptr_eq(&live, mesh))
    }
}

/// At most `CAPACITY` values, each held against one live mesh and one key.
pub(crate) struct MeshMemo<K, V, const CAPACITY: usize> {
    entries: Vec<MemoEntry<K, V>>,
}

impl<K: PartialEq, V, const CAPACITY: usize> MeshMemo<K, V, CAPACITY> {
    /// An empty memo.
    pub(crate) const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Number of live entries. Test hook for the capacity bound.
    pub(crate) fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Drop every entry.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    /// Run `read` against the value for (`mesh`, `key`), if there is a live
    /// entry for it.
    pub(crate) fn read<T>(
        &self,
        mesh: &Arc<TriangleMesh>,
        key: &K,
        read: impl FnOnce(&V) -> Option<T>,
    ) -> Option<T> {
        self.entries
            .iter()
            .find(|entry| entry.matches(mesh, key))
            .and_then(|entry| read(&entry.value))
    }

    /// Insert or update the value for (`mesh`, `key`) through `fill`.
    ///
    /// `empty` builds the value a fresh entry starts from. For a cache whose
    /// entry holds several independently built slots, this is what lets one
    /// slot be written without discarding the others.
    pub(crate) fn write(
        &mut self,
        mesh: &Arc<TriangleMesh>,
        key: K,
        empty: impl FnOnce() -> V,
        fill: impl FnOnce(&mut V),
    ) {
        self.sweep();
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.matches(mesh, &key))
        {
            fill(&mut entry.value);
            return;
        }
        self.evict();
        let mut value = empty();
        fill(&mut value);
        self.entries.push(MemoEntry {
            mesh: Arc::downgrade(mesh),
            key,
            value,
        });
    }

    /// Sweep entries whose mesh is gone, so a closed model's value is released
    /// at the next insert rather than held for the life of the process.
    fn sweep(&mut self) {
        self.entries.retain(|entry| entry.mesh.strong_count() > 0);
    }

    /// Make room for one more entry, oldest first.
    fn evict(&mut self) {
        while self.entries.len() >= CAPACITY {
            self.entries.remove(0);
        }
    }
}

impl<K: PartialEq, V: Clone, const CAPACITY: usize> MeshMemo<K, V, CAPACITY> {
    /// The value for (`mesh`, `key`), or `None`.
    pub(crate) fn get(&self, mesh: &Arc<TriangleMesh>, key: &K) -> Option<V> {
        self.read(mesh, key, |value| Some(value.clone()))
    }

    /// Insert or replace the value for (`mesh`, `key`).
    pub(crate) fn put(&mut self, mesh: &Arc<TriangleMesh>, key: K, value: V) {
        self.sweep();
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.matches(mesh, &key))
        {
            entry.value = value;
            return;
        }
        self.evict();
        self.entries.push(MemoEntry {
            mesh: Arc::downgrade(mesh),
            key,
            value,
        });
    }
}
