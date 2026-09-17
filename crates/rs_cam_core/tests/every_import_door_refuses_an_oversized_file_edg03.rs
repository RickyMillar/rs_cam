//! EDG-03: every import door refuses an oversized file before it parses it.
//!
//! STEP has carried `FileTooLarge` since it landed. DXF and SVG read the
//! whole file into a third-party parser with no limit, so one stray
//! multi-gigabyte file could exhaust memory in a dependency. The three
//! doors now share `io::file_size_over_limit` and each keeps its own error
//! type.
//!
//! The fixtures are **sparse** files: `File::set_len` gives the file the
//! declared length without writing a byte, so the case costs no disk and
//! runs in milliseconds.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use rs_cam_core::io::MAX_IMPORT_FILE_SIZE;
use rs_cam_core::io::dxf_input::{DxfError, load_dxf, load_dxf_full};
use rs_cam_core::io::svg_input::{SvgError, load_svg};

/// A sparse file one byte over the import limit, removed on drop.
struct OversizeFixture {
    path: PathBuf,
}

impl OversizeFixture {
    fn new(extension: &str) -> Self {
        // The cases run in parallel. A shared name would let one case
        // delete the file another case is reading.
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let name = format!(
            "rs_cam_edg03_{}_{}.{extension}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        let file = File::create(&path).expect("create the sparse fixture");
        file.set_len(MAX_IMPORT_FILE_SIZE + 1)
            .expect("declare the fixture length");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for OversizeFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[test]
fn the_fixture_really_is_over_the_limit() {
    // Non-vacuity anchor: a filesystem that refuses `set_len` would make
    // every case below pass for the wrong reason.
    let fixture = OversizeFixture::new("dxf");
    let len = std::fs::metadata(fixture.path()).unwrap().len();
    assert!(
        len > MAX_IMPORT_FILE_SIZE,
        "the sparse fixture is {len} bytes, which is not over the \
         {MAX_IMPORT_FILE_SIZE} byte limit"
    );
}

#[test]
fn dxf_import_refuses_an_oversized_file() {
    let fixture = OversizeFixture::new("dxf");
    let error = load_dxf(fixture.path(), 5.0).expect_err("DXF must refuse");
    let DxfError::FileTooLarge { size_mb, limit_mb } = error else {
        panic!("DXF refused with {error:?}, not FileTooLarge");
    };
    assert_eq!(limit_mb, MAX_IMPORT_FILE_SIZE / (1024 * 1024));
    assert!(
        size_mb >= limit_mb,
        "the reported size must reach the limit"
    );
}

#[test]
fn the_dxf_drill_target_door_refuses_it_too() {
    // `load_dxf_full` is the door the drill picker uses. It carries the
    // guard; `load_dxf` only wraps it.
    let fixture = OversizeFixture::new("dxf");
    let error = load_dxf_full(fixture.path(), 5.0).expect_err("DXF must refuse");
    assert!(
        matches!(error, DxfError::FileTooLarge { .. }),
        "load_dxf_full refused with {error:?}, not FileTooLarge"
    );
}

#[test]
fn svg_import_refuses_an_oversized_file() {
    let fixture = OversizeFixture::new("svg");
    let error = load_svg(fixture.path(), 0.1).expect_err("SVG must refuse");
    let SvgError::FileTooLarge { size_mb, limit_mb } = error else {
        panic!("SVG refused with {error:?}, not FileTooLarge");
    };
    assert_eq!(limit_mb, MAX_IMPORT_FILE_SIZE / (1024 * 1024));
    assert!(
        size_mb >= limit_mb,
        "the reported size must reach the limit"
    );
}

#[test]
fn the_three_doors_state_the_same_refusal() {
    // The error text is the operator's only report. STEP set the shape;
    // DXF and SVG match it.
    let dxf = OversizeFixture::new("dxf");
    let svg = OversizeFixture::new("svg");
    let dxf_text = load_dxf(dxf.path(), 5.0)
        .expect_err("DXF must refuse")
        .to_string();
    let svg_text = load_svg(svg.path(), 0.1)
        .expect_err("SVG must refuse")
        .to_string();
    for text in [&dxf_text, &svg_text] {
        assert!(
            text.contains("too large") && text.contains("limit"),
            "the refusal must say what it refused and against what: {text}"
        );
    }
    assert_eq!(
        dxf_text.replace("DXF", "SVG"),
        svg_text,
        "the two refusals differ by more than the format name"
    );
}
