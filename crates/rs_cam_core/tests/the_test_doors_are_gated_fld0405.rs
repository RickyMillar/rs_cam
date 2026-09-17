//! FLD-04 + FLD-05 — the `maps/` and `surface/` test doors stay behind the
//! `test-support` feature, and no product file reaches them.
//!
//! # What this file guards
//!
//! Twelve items in `maps/` and `surface/` exist for the harnesses alone:
//!
//! - the four cache-stats types and their four `stats()` readers
//!   (`FinishSurfaceCacheStats`, `GeomCacheStats`, `ReachMapCacheStats`,
//!   `TierMapCacheStats`),
//! - `maps::reach_map::reach_map_for_mesh` — a raw constructor that bypasses
//!   the cache, beside the canonical `compute_reach_map`,
//! - `maps::tier_map::reset_drop_call_count`,
//! - `maps::tier_map::TierMap::label_at`,
//! - `surface::slope::GridZ::is_covered`.
//!
//! No product path reads any of them. Each one is bound by an integration-test
//! crate, so `#[cfg(test)]` cannot hide them: FLD-04 + FLD-05 puts them behind
//! the `test-support` cargo feature and gives the eight binding harnesses a
//! `required-features = ["test-support"]` row in `Cargo.toml`. `rs_cam_viz`
//! enables the feature from its `[dev-dependencies]` for
//! `crates/rs_cam_viz/tests/reach_overlay_p5.rs`.
//!
//! The gate holds only while two things stay true, so this file asserts both.
//!
//! # The two arms
//!
//! 1. Every one of the twelve declarations carries
//!    `#[cfg(feature = "test-support")]` in the attribute run directly above
//!    it. A declaration that loses the attribute puts the door back into the
//!    default public API of the crate.
//! 2. No `.rs` file under `crates/rs_cam_core/src`, outside the seven files
//!    that declare the twelve, names one of the eight identifiers in code. A
//!    comment line may name them; prose costs no build.
//!
//! # Non-vacuity
//!
//! Arm 1 counts the declarations it accounted for and fails below twelve.
//! Arm 2 walks at least [`MIN_FILES_WALKED`] files, and it runs the same
//! matcher over the seven owner files first: that scan must find at least
//! [`MIN_OWNER_HITS`] hits, or the matcher reads nothing and the empty result
//! outside the owners means nothing.
//!
//! # Known limit of arm 2
//!
//! `label_at` and `is_covered` are method names, not paths. A future,
//! unrelated method of either name anywhere under `src/` reads as a hit here.
//! That is the same trade `the_research_arms_are_feature_gated_fin01.rs`
//! makes: a false red that a reader resolves in one line beats a silent gap.
//!
//! # Red before the fix
//!
//! Arm 1 read `surface/slope.rs` with the `#[cfg]` line above
//! `pub fn is_covered` deleted, and named that one ungated declaration. Arm 2
//! read a `pub(crate) fn probe_teeth(z: GridZ) -> bool { z.is_covered() }`
//! appended to `maps/grid.rs`, and named `maps/grid.rs:107`. Both defects
//! were injected, both arms went red, and both files were restored.
//!
//! The teeth check for arm 2 ran under `--features test-support`, and it has
//! to: with the default features the product reader stops the crate compiling
//! before a test can run. Arm 1 goes red either way, because removing a
//! `#[cfg]` line compiles under both feature sets.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The attribute every test door carries.
const GATE: &str = "#[cfg(feature = \"test-support\")]";

/// One gated declaration: the file under `src/`, and the declaration line as
/// it appears there, trimmed.
struct Door {
    file: &'static str,
    declaration: &'static str,
}

/// The twelve declarations arm 1 checks.
const DOORS: &[Door] = &[
    Door {
        file: "maps/finish_surface_cache.rs",
        declaration: "pub struct FinishSurfaceCacheStats {",
    },
    Door {
        file: "maps/finish_surface_cache.rs",
        declaration: "pub fn stats() -> FinishSurfaceCacheStats {",
    },
    Door {
        file: "maps/geom_cache.rs",
        declaration: "pub struct GeomCacheStats {",
    },
    Door {
        file: "maps/geom_cache.rs",
        declaration: "pub fn stats() -> GeomCacheStats {",
    },
    Door {
        file: "maps/reach_map_cache.rs",
        declaration: "pub struct ReachMapCacheStats {",
    },
    Door {
        file: "maps/reach_map_cache.rs",
        declaration: "pub fn stats() -> ReachMapCacheStats {",
    },
    Door {
        file: "maps/tier_map_cache.rs",
        declaration: "pub struct TierMapCacheStats {",
    },
    Door {
        file: "maps/tier_map_cache.rs",
        declaration: "pub fn stats() -> TierMapCacheStats {",
    },
    Door {
        file: "maps/reach_map.rs",
        declaration: "pub fn reach_map_for_mesh(",
    },
    Door {
        file: "maps/tier_map.rs",
        declaration: "pub fn reset_drop_call_count() {",
    },
    Door {
        file: "maps/tier_map.rs",
        declaration: "pub fn label_at(&self, row: usize, col: usize) -> Option<u8> {",
    },
    Door {
        file: "surface/slope.rs",
        declaration: "pub fn is_covered(self) -> bool {",
    },
];

/// The eight identifiers arm 2 looks for.
const NEEDLES: &[&str] = &[
    "FinishSurfaceCacheStats",
    "GeomCacheStats",
    "ReachMapCacheStats",
    "TierMapCacheStats",
    "reach_map_for_mesh",
    "reset_drop_call_count",
    "label_at",
    "is_covered",
];

/// The files that declare the twelve. Arm 2 skips them and measures its own
/// matcher on them.
const OWNERS: &[&str] = &[
    "maps/finish_surface_cache.rs",
    "maps/geom_cache.rs",
    "maps/reach_map.rs",
    "maps/reach_map_cache.rs",
    "maps/tier_map.rs",
    "maps/tier_map_cache.rs",
    "surface/slope.rs",
];

/// The lowest number of files arm 2 must visit. `src/` held 314 `.rs` files on
/// 2026-09-17. A walk that reads a handful proves nothing.
const MIN_FILES_WALKED: usize = 250;

/// The lowest number of in-owner hits the matcher must find. Each of the eight
/// identifiers is named at least once by its own declaration, and the four
/// cache-stats types are named again by the `stats()` return type.
const MIN_OWNER_HITS: usize = 12;

/// This crate's `src` directory.
fn src_root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(root.is_dir(), "{} no longer exists", root.display());
    root
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Whether `line` carries prose.
fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// Whether `haystack` names `needle` as a whole identifier.
///
/// `relabel_at_seam` contains `label_at` but is a different name, so a plain
/// `contains` would read a false hit.
fn names_identifier(haystack: &str, needle: &str) -> bool {
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut from = 0_usize;
    while let Some(offset) = haystack[from..].find(needle) {
        let start = from + offset;
        let end = start + needle.len();
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word(c));
        let after_ok = haystack[end..].chars().next().is_none_or(|c| !is_word(c));
        if before_ok && after_ok {
            return true;
        }
        from = end;
    }
    false
}

/// The file's path under `src/`, with `/` separators.
fn relative(path: &Path) -> String {
    path.strip_prefix(src_root())
        .unwrap_or_else(|_| panic!("{} is not under src/", path.display()))
        .to_string_lossy()
        .replace('\\', "/")
}

/// Every code line in `path` that names one of the eight identifiers.
fn hits(path: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for (number, raw) in read(path).lines().enumerate() {
        if is_comment(raw) {
            continue;
        }
        for needle in NEEDLES {
            if names_identifier(raw, needle) {
                out.push(format!("{}:{}: {}", relative(path), number + 1, raw.trim()));
                break;
            }
        }
    }
    out
}

// ── arm 1: the gate on the declarations ─────────────────────────────────

/// ARM 1. Every test door sits under `#[cfg(feature = "test-support")]`.
///
/// The attribute may sit anywhere in the contiguous run of `#[...]` lines
/// directly above the declaration, so a door keeps its `#[must_use]` and its
/// `#[derive]` without an ordering rule.
#[test]
fn every_test_door_carries_the_feature_gate() {
    let root = src_root();
    let mut accounted = 0_usize;
    let mut ungated: Vec<String> = Vec::new();

    for door in DOORS {
        let path = root.join(door.file);
        let text = read(&path);
        let lines: Vec<&str> = text.lines().collect();
        let sites: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.trim() == door.declaration)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(
            sites.len(),
            1,
            "`{}` must be declared exactly once in {}. I read {} \
             declaration(s). If the door was renamed or moved, update DOORS",
            door.declaration,
            door.file,
            sites.len()
        );

        let mut index = sites[0];
        let mut gated = false;
        while index > 0 {
            let above = lines[index - 1].trim();
            if !above.starts_with("#[") {
                break;
            }
            if above == GATE {
                gated = true;
            }
            index -= 1;
        }
        if !gated {
            ungated.push(format!(
                "{}:{}: {}",
                door.file,
                sites[0] + 1,
                door.declaration
            ));
        }
        accounted += 1;
    }

    assert_eq!(
        accounted,
        DOORS.len(),
        "all {} doors must be accounted for, or this arm asserts nothing. I \
         accounted for {accounted}",
        DOORS.len()
    );
    assert!(
        ungated.is_empty(),
        "FLD-04 + FLD-05: every `maps/` and `surface/` test door sits behind \
         `{GATE}`. No product path reads them, and an ungated door puts a \
         test seam back into the default public API of this crate. The eight \
         binding harnesses carry `required-features = [\"test-support\"]` in \
         `Cargo.toml`. I read {} ungated declaration(s):\n  {}",
        ungated.len(),
        ungated.join("\n  ")
    );
}

// ── arm 2: no product reader ────────────────────────────────────────────

/// ARM 2. No product file under `src/` names a test door in code.
///
/// The gate only holds while this stays true: one call from a module that is
/// always compiled makes the default build fail instead of shrinking, and the
/// door is then a product API that the gate hides by accident.
#[test]
fn no_product_file_names_a_test_door() {
    let mut files = Vec::new();
    collect_rs(&src_root(), &mut files);
    files.sort();
    assert!(
        files.len() >= MIN_FILES_WALKED,
        "the walk must visit at least {MIN_FILES_WALKED} files, or this arm \
         passes on a tree it never read. I read {}",
        files.len()
    );

    let mut owner_hits = 0_usize;
    let mut outside: Vec<String> = Vec::new();
    for path in &files {
        let rel = relative(path);
        if OWNERS.contains(&rel.as_str()) {
            owner_hits += hits(path).len();
        } else {
            outside.extend(hits(path));
        }
    }

    assert!(
        owner_hits >= MIN_OWNER_HITS,
        "the matcher must read the owner files themselves, or an empty result \
         outside them proves nothing. I read {owner_hits} owner hit(s), and at \
         least {MIN_OWNER_HITS} are there: every door names itself"
    );
    assert!(
        outside.is_empty(),
        "FLD-04 + FLD-05: the four cache-stats types, their four `stats()` \
         readers, `reach_map_for_mesh`, `reset_drop_call_count`, \
         `TierMap::label_at` and `GridZ::is_covered` are test doors behind the \
         `test-support` feature. A product file that names one in code either \
         breaks the default build or turns the door into a hidden product API. \
         Use `compute_reach_map` for a reach map, or promote the door to a \
         plain `pub` item with a product reader and a sentry. I read {} \
         line(s):\n  {}",
        outside.len(),
        outside.join("\n  ")
    );
}
