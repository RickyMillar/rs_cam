//! D3: a hover or a selection change rewrites the face state words, not the
//! STEP mesh.
//!
//! # The claims
//!
//! 1. The face mesh slot (`app/gpu_upload/meshes.rs`) reads no hover and no
//!    selection. Its key, `EnrichedUploadKey`, has no hover field and no
//!    selection field. So a pointer move cannot rebuild a mesh.
//! 2. `FaceStateKey` holds the mesh key, the hover and the selection. So a
//!    mesh rebuild also rewrites the words.
//! 3. `app/gpu_upload/face_state.rs` is the only file that writes a
//!    `face_state` buffer, and `upload_gpu_data` runs it after
//!    `meshes::upload`.
//! 4. The hover names its model: `last_hover_face` takes the `model_id` of
//!    `PickHit::ModelFace`, so one face index on model A does not light
//!    model B.
//!
//! `upload_gpu_data` needs an `eframe::Frame` with a wgpu render state, so
//! no test can run it. The claims are source arms. Each arm has an anchor
//! that must be present, so an arm cannot pass on a moved or empty file.
//! The key arm (`hover_moves_the_face_state_key_only` in
//! `render/upload_cache.rs`) and the word arm (`face_state.rs` tests) are
//! the behavioural halves.
//!
//! Red proof (run once, red, reverted): `build_enriched` in `meshes.rs`
//! wrote a `hovered_face` word into `gpu_data.face_state` with
//! `write_buffer`. The mesh arm named `hover`, and the writer arm named
//! `meshes.rs` as a second writer. A hover field put back in
//! `EnrichedUploadKey` does not compile until `meshes.rs` keys it, and the
//! mesh arm then names that read.

#![expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn source(relative: &str) -> String {
    let path = root().join(relative);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(text.len() > 500, "{} is too short", path.display());
    text
}

/// The text without its `//` comments, so a comment cannot satisfy or
/// break an arm.
fn code(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The body of `pub struct <name> {`, up to the first `}` line.
fn struct_body(text: &str, name: &str) -> String {
    let open = format!("pub struct {name} {{");
    let start = text
        .find(&open)
        .unwrap_or_else(|| panic!("anchor `{open}` is missing"));
    let rest = &text[start + open.len()..];
    let end = rest
        .find("\n}")
        .unwrap_or_else(|| panic!("`{name}` has no closing brace"));
    rest[..end].to_owned()
}

/// Every `.rs` file under `src/`, as (relative path, code).
fn all_sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries =
            std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(&root().join("src"), &mut files);
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let relative = path
                .strip_prefix(root())
                .map_or_else(|_| path.display().to_string(), |p| p.display().to_string());
            (relative, code(&text))
        })
        .collect()
}

#[test]
fn the_mesh_slot_reads_no_hover_and_no_selection_d3() {
    let meshes = code(&source("src/app/gpu_upload/meshes.rs"));
    assert!(
        meshes.contains("face_mesh_gpu_data("),
        "anchor: meshes.rs no longer builds the face meshes"
    );
    for needle in ["hover", "selected", "selection_report", "highlight_report"] {
        assert!(
            !meshes.contains(needle),
            "meshes.rs reads `{needle}`: a hover or a selection change would rebuild a mesh"
        );
    }

    let cache = code(&source("src/render/upload_cache.rs"));
    let mesh_key = struct_body(&cache, "EnrichedUploadKey");
    assert!(
        mesh_key.contains("meshes:"),
        "anchor: EnrichedUploadKey holds the meshes"
    );
    for needle in ["hover", "selected"] {
        assert!(
            !mesh_key.contains(needle),
            "EnrichedUploadKey has a `{needle}` field: a hover or a selection change moves the mesh key"
        );
    }
}

#[test]
fn the_face_state_key_holds_the_mesh_key_d3() {
    let cache = code(&source("src/render/upload_cache.rs"));
    let state_key = struct_body(&cache, "FaceStateKey");
    for needle in [
        "mesh: EnrichedUploadKey",
        "hovered: Option<(",
        "selected: Vec<(",
    ] {
        assert!(
            state_key.contains(needle),
            "FaceStateKey does not hold `{needle}`"
        );
    }
}

#[test]
fn one_file_writes_the_face_state_words_d3() {
    let writers: Vec<String> = all_sources()
        .into_iter()
        .filter(|(_, code)| {
            code.split("write_buffer(").skip(1).any(|call| {
                call.split(')')
                    .next()
                    .is_some_and(|args| args.contains(".face_state"))
            })
        })
        .map(|(path, _)| path)
        .collect();
    assert_eq!(
        writers,
        vec!["src/app/gpu_upload/face_state.rs".to_owned()],
        "the face state words have one writer"
    );

    let upload = code(&source("src/app/gpu_upload.rs"));
    let meshes_at = upload
        .find("meshes::upload(")
        .unwrap_or_else(|| panic!("anchor: upload_gpu_data no longer calls meshes::upload"));
    let state_at = upload
        .find("face_state::upload(")
        .unwrap_or_else(|| panic!("upload_gpu_data does not call face_state::upload"));
    assert!(
        meshes_at < state_at,
        "face_state::upload must run after meshes::upload, so a rebuilt mesh gets its words"
    );
}

#[test]
fn the_hover_names_its_model_d3() {
    let viewport = code(&source("src/app/viewport.rs"));
    let anchor = "self.last_hover_face = match";
    let at = viewport
        .find(anchor)
        .unwrap_or_else(|| panic!("anchor `{anchor}` is missing in viewport.rs"));
    // The match arms up to the first `_ => None`: the hover write.
    let arms = &viewport[at..];
    let arms = &arms[..arms.find("_ => None").unwrap_or(arms.len())];
    assert!(
        arms.contains("PickHit::ModelFace { model_id, face_id }"),
        "the hover write does not read both fields of PickHit::ModelFace"
    );
    assert!(
        arms.contains("Some((*model_id, *face_id))"),
        "last_hover_face does not take the model id"
    );
}
