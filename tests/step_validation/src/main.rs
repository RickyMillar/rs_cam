/// Phase 0: Validate that truck-stepio can parse STEP files and tessellate faces.
///
/// This is a throwaway validation crate — NOT a workspace member.
/// Run with: cd tests/step_validation && cargo run
use truck_stepio::r#in::Table;
use truck_meshalgo::tessellation::MeshedShape;
use truck_meshalgo::tessellation::MeshableShape;
use truck_polymesh::PolygonMesh;

fn main() {
    let fixtures_dir = std::path::Path::new("../../crates/rs_cam_core/tests/fixtures/step");

    if !fixtures_dir.exists() {
        eprintln!("No fixtures directory at {:?}", fixtures_dir);
        return;
    }

    let mut files: Vec<_> = std::fs::read_dir(fixtures_dir)
        .expect("read fixtures dir")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            matches!(
                p.extension().and_then(|s| s.to_str()),
                Some("step") | Some("stp")
            )
        })
        .collect();
    files.sort_by_key(|e| e.file_name());

    if files.is_empty() {
        eprintln!("No .step/.stp files found in {:?}", fixtures_dir);
        return;
    }

    println!("=== truck STEP validation ===\n");

    let mut pass = 0;
    let mut fail = 0;

    for entry in &files {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();
        print!("{name}: ");

        let step_string = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                println!("FAIL (read error: {e})");
                fail += 1;
                continue;
            }
        };

        let table = match Table::from_step(&step_string) {
            Some(t) => t,
            None => {
                println!("FAIL (parse returned None)");
                fail += 1;
                continue;
            }
        };

        // Count shells
        let shell_count = table.shell.len();
        if shell_count == 0 {
            println!("FAIL (no shells found)");
            fail += 1;
            continue;
        }

        // Try to tessellate each shell
        let mut total_faces = 0;
        let mut total_tris = 0;
        let mut tess_ok = true;

        for (idx, step_shell) in table.shell.values().enumerate() {
            match table.to_compressed_shell(step_shell) {
                Ok(cshell) => {
                    let face_count = cshell.faces.len();
                    total_faces += face_count;

                    let poly: PolygonMesh = match std::panic::catch_unwind(
                        std::panic::AssertUnwindSafe(|| {
                            cshell.triangulation(0.1).to_polygon()
                        }),
                    ) {
                        Ok(p) => p,
                        Err(_) => {
                            println!("FAIL (tessellation panicked on shell {idx})");
                            tess_ok = false;
                            break;
                        }
                    };

                    let tri_count = poly.tri_faces().len() + poly.quad_faces().len() * 2;
                    total_tris += tri_count;
                }
                Err(e) => {
                    println!("FAIL (to_compressed_shell error on shell {idx}: {e:?})");
                    tess_ok = false;
                    break;
                }
            }
        }

        if tess_ok {
            println!("OK — {shell_count} shell(s), {total_faces} faces, {total_tris} triangles");
            pass += 1;
        } else {
            fail += 1;
        }
    }

    println!("\n=== Results: {pass} pass, {fail} fail out of {} files ===", pass + fail);
    if fail > 0 {
        std::process::exit(1);
    }
}
