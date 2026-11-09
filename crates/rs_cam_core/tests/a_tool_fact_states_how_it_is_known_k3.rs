//! K3 (feeds contract P6, rulings Q7 and Q9): every tool fact states how it
//! is known.
//!
//! # What is asserted
//!
//! (a) A bare fact (`stickout = 30.0`) refuses to load. ONE refusal names
//!     every bare key in the file and the three forms. Red before P6: the
//!     file loaded.
//! (b) Each form (`stated`, `catalogue`, `estimated`, an absent key)
//!     round-trips through a save and a load; an absent key writes no key.
//! (c) A tool with no centre-cutting key loads as `Unknown`, not as a
//!     centre-cutting tool, and the save of "centre cutting" and of "not
//!     stated" now differ (C 1.4). Red before P6: `None` meant centre
//!     cutting.
//! (d) The old key `non_centre_cutting` refuses as retired.
//! (e) The tool library takes the same forms and refuses a bare fact.

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};

use rs_cam_core::compute::tool_config::{CentreCutting, FactKind, ToolFact};
use rs_cam_core::session::{ProjectFile, ProjectSession, SessionError};

const HEAD: &str = r#"format_version = 3

[job]
name = "K3"

[job.stock]
x = 40.0
y = 40.0
z = 6.0
"#;

fn project(tool_body: &str) -> String {
    format!(
        "{HEAD}\n[[tools]]\nid = 0\nname = \"6 mm End Mill\"\ntype = \"end_mill\"\ndiameter = 6.0\n{tool_body}"
    )
}

fn scratch(tag: &str) -> PathBuf {
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("k3_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn load_text(text: &str) -> Result<ProjectSession, SessionError> {
    let project = ProjectFile::from_toml_str(text)?;
    ProjectSession::from_project_file(project, Path::new("."))
}

/// Load `text`, save it, and give the saved text.
fn saved(tag: &str, text: &str) -> String {
    let dir = scratch(tag);
    let src = dir.join("in.toml");
    std::fs::write(&src, text).unwrap();
    let out = dir.join("out.toml");
    ProjectSession::load(&src).unwrap().save(&out).unwrap();
    let saved = std::fs::read_to_string(&out).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    saved
}

#[test]
fn a_bare_fact_refuses_and_names_every_bare_key_k3_a() {
    let text = project("stickout = 30.0\nflute_count = 2\ncutting_length = { stated = 20.0 }\n");
    let Err(err) = load_text(&text) else {
        panic!("a bare tool fact must refuse to load");
    };
    let SessionError::BareToolFacts(paths) = &err else {
        panic!("expected SessionError::BareToolFacts, got {err}");
    };
    assert_eq!(
        paths,
        &vec![
            "tools[0].stickout".to_owned(),
            "tools[0].flute_count".to_owned()
        ]
    );
    let message = err.to_string();
    for needle in [
        "stickout",
        "flute_count",
        "stated",
        "catalogue",
        "estimated",
    ] {
        assert!(
            message.contains(needle),
            "the refusal omits `{needle}`: {message}"
        );
    }
}

#[test]
fn each_form_round_trips_through_save_and_load_k3_b() {
    let text = project(
        "cutting_length = { catalogue = 8.0, source = \"a catalogue row\" }\n\
         stickout = { stated = 30.0 }\n\
         flute_count = { estimated = 2, basis = \"from the name\" }\n",
    );
    let session = load_text(&text).unwrap();
    let tool = &session.tools()[0];
    assert_eq!(
        tool.cutting_length_fact(),
        &ToolFact::Catalogue {
            value: 8.0,
            source: "a catalogue row".to_owned()
        }
    );
    assert_eq!(tool.stickout_fact(), &ToolFact::Stated(30.0));
    assert_eq!(tool.flute_count_fact().kind(), FactKind::Estimated);
    assert!(tool.shank_diameter_fact().is_unknown());

    let first = saved("b1", &text);
    assert!(
        !first.contains("shank_diameter"),
        "an Unknown fact writes no key:\n{first}"
    );
    let second = saved("b2", &first);
    assert_eq!(first, second, "a load and a save keep every fact");
    let back = load_text(&second).unwrap();
    let again = &back.tools()[0];
    assert_eq!(again.cutting_length_fact(), tool.cutting_length_fact());
    assert_eq!(again.stickout_fact(), tool.stickout_fact());
    assert_eq!(again.flute_count_fact(), tool.flute_count_fact());
    assert!(again.shank_diameter_fact().is_unknown());
}

#[test]
fn an_absent_centre_cutting_key_is_unknown_not_centre_cutting_k3_c() {
    let unknown = load_text(&project("")).unwrap();
    assert!(unknown.tools()[0].centre_cutting_fact().is_unknown());
    assert!(unknown.tools()[0].non_centre_cutting_geometry().is_none());

    let yes_text = project("centre_cutting = { stated = \"yes\" }\n");
    let yes = load_text(&yes_text).unwrap();
    assert_eq!(
        yes.tools()[0].centre_cutting_fact(),
        &ToolFact::Stated(CentreCutting::Yes)
    );
    let no = load_text(&project(
        "centre_cutting = { stated = \"no\", centre_core_diameter_mm = 2.4, blade_protrusion_mm = 0.5 }\n",
    ))
    .unwrap();
    assert!(no.tools()[0].non_centre_cutting_geometry().is_some());

    // C 1.4: the save of "centre cutting" and of "not stated" differ.
    let saved_unknown = saved("c1", &project(""));
    let saved_yes = saved("c2", &yes_text);
    assert_ne!(saved_unknown, saved_yes);
    assert!(!saved_unknown.contains("centre_cutting"));
    assert!(saved_yes.contains("centre_cutting"));
}

#[test]
fn the_old_non_centre_cutting_key_refuses_as_retired_k3_d() {
    let text = project(
        "\n[tools.non_centre_cutting]\ncentre_core_diameter_mm = 2.4\nblade_protrusion_mm = 0.5\n",
    );
    let Err(err) = load_text(&text) else {
        panic!("the retired key must refuse");
    };
    let SessionError::RetiredKeys(found) = &err else {
        panic!("expected SessionError::RetiredKeys, got {err}");
    };
    assert!(
        found.iter().any(|f| f.path.contains("non_centre_cutting")),
        "{err}"
    );
    assert!(err.to_string().contains("centre_cutting"), "{err}");
}

#[test]
fn the_tool_library_takes_the_same_forms_k3_e() {
    use rs_cam_core::io::tool_library::{ToolLibraryError, load_from};
    let dir = scratch("e");
    let base = "[[tools]]\nid = 0\nname = \"t\"\ntool_number = 1\ntool_type = \"end_mill\"\n\
                diameter = 6.0\ncorner_radius = 0.0\nincluded_angle = 0.0\nshaft_diameter = 6.0\n\
                tool_material = \"carbide\"\ncut_direction = \"up_cut\"\nvendor = \"\"\n\
                product_id = \"\"\n";
    std::fs::write(
        dir.join("good.toml"),
        format!("{base}stickout = {{ catalogue = 30.0, source = \"a row\" }}\n"),
    )
    .unwrap();
    let good = load_from(&dir, "good").unwrap();
    assert_eq!(good.tools[0].stickout_fact().kind(), FactKind::Catalogue);
    assert!(good.tools[0].cutting_length_fact().is_unknown());

    std::fs::write(dir.join("bare.toml"), format!("{base}stickout = 30.0\n")).unwrap();
    match load_from(&dir, "bare") {
        Err(ToolLibraryError::ToolFacts { text, .. }) => {
            assert!(text.contains("tools[0].stickout"), "{text}");
        }
        other => panic!("a bare fact in a catalog must refuse, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
