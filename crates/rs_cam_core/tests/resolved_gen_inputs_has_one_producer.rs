//! WP11a sentry — `ResolvedGenInputs` is public, its fields are private, and
//! one public function produces it.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §1 "ResolvedGenInputs" and §4 WP11a.
//!
//! The core session resolves every per-generation input into one bundle. The
//! GUI worker assembles a second answer of its own (`ComputeRequest`, 13
//! mirrored fields). The programme publishes the core bundle, so the GUI door
//! reads one answer instead of building a second one. A published type alone
//! does not stop a second assembly. Three constraints do:
//!
//! - the type derives no `Default`;
//! - the type exposes no public field and no public setter;
//! - exactly one public function produces the type.
//!
//! The compiler checks none of those three. A field visibility and a return
//! type are source text, not values a test can read, so this test reads the
//! source, in the idiom of
//! `crates/rs_cam_viz/tests/command_registry_surfaces.rs:34-43`. The struct
//! scan reads one file. The producer scan reads every `.rs` file under this
//! crate's `src/`, because a second producer in another module dodges a
//! single-file scan. The non-vacuity guards fail the test when a scan reads
//! nothing, because an empty population passes and looks healthy.
//!
//! If this test fails because a symbol moved, retarget it — do not delete it.
//!
//! NOT MEASURED here: whether the GUI door calls the resolver. WP10 and WP11b
//! measure that door. This test measures what the core crate publishes.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The core source file that declares the bundle.
const STRUCT_FILE: &str = "src/session/compute.rs";

/// The declaration line the programme requires.
const STRUCT_HEAD: &str = "pub struct ResolvedGenInputs {";

/// The only function that may produce the bundle.
const PRODUCER: &str = "pub fn resolve_generation_inputs(";

/// The return-type spellings that produce the bundle. A `-> Self` inside an
/// `impl` block on the type is the fourth form. The scan reads it apart.
const RETURN_FORMS: [&str; 3] = [
    "-> ResolvedGenInputs",
    "-> Result<ResolvedGenInputs",
    "-> Option<ResolvedGenInputs",
];

/// Read one source file, relative to this crate's manifest.
fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    assert!(
        path.is_file(),
        "scanned path {} no longer exists; retarget this test",
        path.display()
    );
    read(&path)
}

/// Read one source file by path.
fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Collect every `.rs` file under `dir`, recursively.
fn rust_sources_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let listing = std::fs::read_dir(dir);
    let listing = listing.unwrap_or_else(|e| panic!("read dir {}: {e}", dir.display()));
    for entry in listing {
        let entry = entry.unwrap_or_else(|e| panic!("entry in {}: {e}", dir.display()));
        let path = entry.path();
        if path.is_dir() {
            rust_sources_under(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Every `.rs` file under this crate's `src/`, in a stable order.
fn core_sources() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(root.is_dir(), "{} is not a directory", root.display());
    let mut files = Vec::new();
    rust_sources_under(&root, &mut files);
    files.sort();
    files
}

/// A line comment, a doc comment, or a module doc comment.
fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// The nearest `fn` line at or above `at`, trimmed. A return type sits on its
/// own line when the parameter list wraps, so the scan walks back to the name.
fn enclosing_fn(lines: &[&str], at: usize) -> String {
    for index in (0..=at).rev() {
        let line = lines[index];
        if is_comment(line) {
            continue;
        }
        if line.contains("fn ") {
            return line.trim().to_owned();
        }
    }
    panic!("no `fn` line above scan line {}", at + 1);
}

/// One function whose return type names the bundle.
struct Producer {
    file: String,
    signature: String,
}

/// Append every producer that `text` declares to `out`.
fn collect_producers(text: &str, file: &str, out: &mut Vec<Producer>) {
    let lines: Vec<&str> = text.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let line = *line;
        if is_comment(line) {
            continue;
        }
        if RETURN_FORMS.iter().any(|form| line.contains(*form)) {
            out.push(Producer {
                file: file.to_owned(),
                signature: enclosing_fn(&lines, index),
            });
        }
    }
    // A `-> Self` inside an `impl` block on the type names no other form. A
    // builder and a hand-written `Default` both hide there. Scan the blocks.
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let head = line.trim_start();
        if !is_comment(line) && head.starts_with("impl") && head.contains("ResolvedGenInputs") {
            let mut cursor = index + 1;
            while cursor < lines.len() && lines[cursor] != "}" {
                if !is_comment(lines[cursor]) && lines[cursor].contains("-> Self") {
                    out.push(Producer {
                        file: file.to_owned(),
                        signature: enclosing_fn(&lines, cursor),
                    });
                }
                cursor += 1;
            }
            index = cursor;
        }
        index += 1;
    }
}

// ── the published type ─────────────────────────────────────────────────────

/// The bundle is public. Its fields and its derives stay closed, so a caller
/// outside the module names the type and builds no value of it.
#[test]
fn the_bundle_is_public_and_its_fields_are_private() {
    let text = source(STRUCT_FILE);
    assert!(!text.is_empty(), "{STRUCT_FILE} is empty");
    let lines: Vec<&str> = text.lines().collect();

    let at = lines.iter().position(|line| line.trim_end() == STRUCT_HEAD);
    let Some(head) = at else {
        panic!("`{STRUCT_HEAD}` is gone from {STRUCT_FILE}; retarget this test");
    };

    // A derive above the declaration. `Default` builds the bundle out of
    // nothing, which is a second producer.
    for index in (0..head).rev() {
        let attribute = lines[index].trim_start();
        if !attribute.starts_with("#[") && !attribute.starts_with("//") {
            break;
        }
        let default = attribute.starts_with("#[derive(") && attribute.contains("Default");
        assert!(
            !default,
            "ResolvedGenInputs derives Default at {STRUCT_FILE} line {}",
            index + 1
        );
    }

    // The field lines, down to the closing brace at column 0.
    let mut fields = 0_usize;
    let mut cursor = head + 1;
    while cursor < lines.len() && lines[cursor] != "}" {
        let line = lines[cursor];
        let field = line.trim_start();
        if !is_comment(line) && !field.is_empty() {
            let public = field.starts_with("pub ") || field.starts_with("pub(");
            assert!(
                !public,
                "ResolvedGenInputs publishes a field at {STRUCT_FILE} line {}: {field}",
                cursor + 1
            );
            fields += 1;
        }
        cursor += 1;
    }
    assert!(
        cursor < lines.len(),
        "the ResolvedGenInputs body is unterminated in {STRUCT_FILE}"
    );
    assert!(
        fields > 0,
        "the ResolvedGenInputs body declares no field; the scan reads the wrong block"
    );
}

/// Exactly one function produces the bundle, and it is the public resolver.
#[test]
fn one_public_function_produces_the_bundle() {
    let files = core_sources();
    assert!(!files.is_empty(), "the core src scan read no file");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let declaring = root.join(STRUCT_FILE);
    assert!(
        files.contains(&declaring),
        "the core src scan missed {STRUCT_FILE}"
    );

    let mut found = Vec::new();
    for path in &files {
        let text = read(path);
        let short = path.strip_prefix(root).unwrap_or(path.as_path());
        let name = short.display().to_string();
        collect_producers(&text, &name, &mut found);
    }

    let listed = found
        .iter()
        .map(|producer| format!("{} :: {}", producer.file, producer.signature))
        .collect::<Vec<_>>()
        .join("; ");
    assert_eq!(
        found.len(),
        1,
        "one function may produce ResolvedGenInputs; the scan found: [{listed}]"
    );
    assert!(
        found[0].signature.starts_with(PRODUCER),
        "the one producer must be `{PRODUCER}`; the scan found [{listed}]"
    );
}

/// The compile-level arm. Another crate names the type. Before WP11a the
/// declaration is private and this line does not compile.
fn _nameable(_: &rs_cam_core::session::ResolvedGenInputs) {}

/// The published path stays nameable from outside the core crate.
#[test]
fn the_bundle_is_nameable_from_another_crate() {
    let _named: fn(&rs_cam_core::session::ResolvedGenInputs) = _nameable;
}
