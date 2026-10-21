//! F140: every folder `CLAUDE.md` under `src/` names real files, and names
//! every file of its folder.
//!
//! The folder files are the entry point for an agent. A map that names a
//! moved file, or that leaves a file out, sends the reader to the wrong
//! place, and nothing else checks it. The rules are in
//! `common/folder_maps.rs`; `rs_cam_viz/tests/folder_maps_name_real_files_f140.rs`
//! is the twin for the viz folders.
//!
//! Red proof (run once, red, reverted): the token `` `no_such_file.rs` ``
//! added to `tool/CLAUDE.md` made `every_rs_token_resolves_f140` name it; a
//! new empty `tool/stray_f140.rs` made `every_rs_file_is_on_a_map_f140` name
//! it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

#[path = "common/folder_maps.rs"]
mod folder_maps;

use std::path::Path;

/// `(folder relative to src, file name)`: a file a map leaves out on
/// purpose. Keep it short; a name belongs on the map.
const EXCLUDED: &[(&str, &str)] = &[];

/// The fewest folder files the scan must read (26 on 2026-10-21).
const MIN_FOLDER_FILES: usize = 20;

/// The fewest `.rs` tokens that must resolve (several hundred on
/// 2026-10-21).
const MIN_RESOLVED: usize = 200;

fn report() -> folder_maps::MapReport {
    folder_maps::scan_crate(Path::new(env!("CARGO_MANIFEST_DIR")), EXCLUDED)
}

#[test]
fn the_map_reader_reads_the_token_forms_f140() {
    assert_eq!(
        folder_maps::rs_tokens("`a.rs` (x), b/c.rs, sim_*.rs, ext/{size,drill}.rs; d.toml"),
        vec!["a.rs", "b/c.rs", "sim_*.rs", "ext/{size,drill}.rs"]
    );
    assert_eq!(
        folder_maps::expand_braces("ext/{size,drill}.rs"),
        vec!["ext/size.rs", "ext/drill.rs"]
    );
    assert!(folder_maps::glob_match("scallop*.rs", "scallop_math.rs"));
    assert!(folder_maps::glob_match("*_modal.rs", "import_modal.rs"));
    assert!(!folder_maps::glob_match("sim_*.rs", "simulate.rs"));
    assert_eq!(
        folder_maps::backtick_spans("x `a.rs` y `b` z"),
        vec!["a.rs", "b"]
    );
}

#[test]
fn the_scan_is_not_vacuous_f140() {
    let r = report();
    assert!(
        r.folder_files >= MIN_FOLDER_FILES,
        "the scan read {} folder files, fewer than {MIN_FOLDER_FILES}",
        r.folder_files
    );
    assert!(
        r.resolved >= MIN_RESOLVED,
        "only {} `.rs` tokens resolved, fewer than {MIN_RESOLVED}: the reader broke",
        r.resolved
    );
    assert_eq!(
        r.exclusions_used,
        EXCLUDED.len(),
        "an exclusion names no unlisted file; delete it"
    );
}

#[test]
fn every_rs_token_resolves_f140() {
    let r = report();
    assert!(
        r.unresolved.is_empty(),
        "a folder CLAUDE.md names a `.rs` file that does not exist; correct the map: {:#?}",
        r.unresolved
    );
}

#[test]
fn every_rs_file_is_on_a_map_f140() {
    let r = report();
    assert!(
        r.unlisted.is_empty(),
        "a `.rs` file is on no folder CLAUDE.md map; name it there: {:#?}",
        r.unlisted
    );
}
