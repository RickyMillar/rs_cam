//! Feeds contract P11c, D13 item 6 (`planning/feeds_contract_2026-11-03/STATUS.md`,
//! "Lead rulings for P11c"): every machine table states its class
//! (`MachineClass::{Hobby, Industrial}`). `Hobby` has the one default rigidity
//! set; `Industrial` carries no values, so an industrial table with no
//! `[rigidity]` block refuses to load, with a message that says what to do.
//!
//! The tables are written from `MachineProfile::default()` (no number is
//! typed here), then edited: the class is set and the `rigidity` table is
//! removed.
//!
//! Arms, at the machine library door (`io::machine_library::load_from`) and
//! the project door (`ProjectSession::load`, `[job.machine]`):
//!
//! 1. Industrial, no `[rigidity]`: refused, the error names
//!    `INDUSTRIAL_NEEDS_RIGIDITY`.
//! 2. Hobby, no `[rigidity]`: loads with `RigidityProfile::default()`, the
//!    basis `HobbyDefault`.
//! 3. Industrial with its own `[rigidity]`: loads, the basis
//!    `IndustrialStated`.
//! 4. No `class` key: refused (the class is required; break).
//! 5. The deleted keys `woc_roughing_factor`, `woc_roughing_max_mm` and
//!    `woc_finishing_mm`: refused at both doors (break).
//!
//! Red before P11c: `MachineClass` does not exist at HEAD f5170da03 (no
//! compile); at HEAD a table with no `[rigidity]` refused for every machine.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test an_industrial_machine_states_its_rigidity_p11c
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;

use rs_cam_core::machine::{
    INDUSTRIAL_NEEDS_RIGIDITY, MachineClass, MachineProfile, RigidityBasis, RigidityProfile,
};
use rs_cam_core::session::ProjectSession;

/// The default machine as a TOML table, edited by `edit`.
fn machine_table(edit: impl FnOnce(&mut toml::Table)) -> toml::Table {
    let mut t = toml::Table::try_from(MachineProfile::default()).expect("the default serialises");
    edit(&mut t);
    t
}

fn set_class(t: &mut toml::Table, class: &str) {
    t.insert("class".to_owned(), toml::Value::String(class.to_owned()));
}

fn tmp_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("p11c_class_{name}"));
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

/// The machine library door.
fn load_library(name: &str, t: &toml::Table) -> Result<MachineProfile, String> {
    let dir = tmp_dir(name);
    std::fs::write(
        dir.join(format!("{name}.toml")),
        toml::to_string(t).expect("text"),
    )
    .expect("write");
    rs_cam_core::io::machine_library::load_from(&dir, name).map_err(|e| format!("{e:?} / {e}"))
}

/// The project door: a minimal project whose `[job.machine]` is `t`.
fn load_project(name: &str, t: &toml::Table) -> Result<ProjectSession, String> {
    let mut job = toml::Table::new();
    job.insert("name".to_owned(), toml::Value::String(name.to_owned()));
    job.insert("machine".to_owned(), toml::Value::Table(t.clone()));
    let mut root = toml::Table::new();
    root.insert("format_version".to_owned(), toml::Value::Integer(3));
    root.insert("job".to_owned(), toml::Value::Table(job));
    let path = tmp_dir(name).join("project.toml");
    std::fs::write(&path, toml::to_string(&root).expect("text")).expect("write");
    ProjectSession::load(&path).map_err(|e| format!("{e:?} / {e}"))
}

#[test]
fn an_industrial_table_with_no_rigidity_refuses_to_load_p11c() {
    let t = machine_table(|t| {
        set_class(t, "industrial");
        let _ = t.remove("rigidity");
    });
    let lib = load_library("industrial_bare", &t).expect_err("the library refuses it");
    assert!(lib.contains(INDUSTRIAL_NEEDS_RIGIDITY), "library: {lib}");
    let Err(project) = load_project("industrial_bare_project", &t) else {
        panic!("the project door refuses it")
    };
    assert!(
        project.contains(INDUSTRIAL_NEEDS_RIGIDITY),
        "project: {project}"
    );
}

#[test]
fn a_hobby_table_with_no_rigidity_reads_the_hobby_default_p11c() {
    let t = machine_table(|t| {
        set_class(t, "hobby");
        let _ = t.remove("rigidity");
    });
    let m = load_library("hobby_bare", &t).expect("a hobby table loads");
    assert_eq!(m.class, MachineClass::Hobby);
    assert_eq!(m.rigidity, RigidityProfile::default());
    assert_eq!(m.rigidity_basis(), RigidityBasis::HobbyDefault);
    let s = load_project("hobby_bare_project", &t).expect("the project loads");
    assert_eq!(s.machine().rigidity, RigidityProfile::default());
}

#[test]
fn an_industrial_table_with_its_own_rigidity_loads_p11c() {
    let t = machine_table(|t| {
        set_class(t, "industrial");
        if let Some(toml::Value::Table(r)) = t.get_mut("rigidity") {
            let v = r
                .get("doc_roughing_factor")
                .and_then(toml::Value::as_float)
                .expect("the default states it");
            r.insert(
                "doc_roughing_factor".to_owned(),
                toml::Value::Float(2.0 * v),
            );
        }
    });
    let m = load_library("industrial_stated", &t).expect("loads");
    assert_eq!(m.class, MachineClass::Industrial);
    assert_eq!(m.rigidity_basis(), RigidityBasis::IndustrialStated);
    assert!(
        (m.rigidity.doc_roughing_factor - 2.0 * RigidityProfile::default().doc_roughing_factor)
            .abs()
            < 1e-12
    );
}

#[test]
fn a_table_with_no_class_refuses_to_load_p11c() {
    let t = machine_table(|t| {
        let _ = t.remove("class");
    });
    let e = load_library("no_class", &t).expect_err("the class is required");
    assert!(e.contains("class"), "{e}");
}

#[test]
fn the_deleted_rigidity_keys_refuse_p11c() {
    // D13 item 2 deleted `woc_roughing_factor`; review item M11 deleted
    // `woc_roughing_max_mm` and `woc_finishing_mm` (none had a reader).
    for key in [
        "woc_roughing_factor",
        "woc_roughing_max_mm",
        "woc_finishing_mm",
    ] {
        let t = machine_table(|t| {
            if let Some(toml::Value::Table(r)) = t.get_mut("rigidity") {
                r.insert(key.to_owned(), toml::Value::Float(0.5));
            }
        });
        let e = load_library(&format!("old_{key}"), &t).expect_err("the key is deleted");
        assert!(e.contains(key), "{e}");
        // The project door names the retired key and the fix (`RETIRED_KEYS`).
        let Err(p) = load_project(&format!("old_{key}_project"), &t) else {
            panic!("the project door refuses the deleted key {key}")
        };
        assert!(p.contains(key) && p.contains("P11c"), "{p}");
    }
}
