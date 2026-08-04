//! **R2 / H2 — the `cavalier_contours` `Shape` failure, reproduced under
//! controlled input, and the mapping it currently receives.**
//!
//! `polygon::offset_one` contains a `catch_unwind` whose `Err` arm returns
//! `Vec::new()` with a `tracing::warn!`. Every 2D consumer reads an empty
//! `Vec` as "the polygon collapsed" and completes successfully with an empty
//! or truncated toolpath. So a library panic, a `< 3`-vertex guard, and a
//! genuine geometric collapse are **the same observable event** to every
//! caller and to the operator.
//!
//! This file is the instrument for that. It is deliberately *not* a gate on
//! the panic: whether cavalier panics is the library's business and R1
//! already decided containment was the only available fix. What it gates is
//! the part that is ours:
//!
//! | test | asserts |
//! |---|---|
//! | `the_captured_panic_asset_still_reaches_cavalier_unrepaired` | the R1 asset is still the input class it was captured as — a fixture that no longer contains the mechanism cannot evidence anything |
//! | `no_hostile_input_escapes_the_offset_chokepoint_as_a_panic` | the containment holds across the whole R2 fixture library, not just the one captured asset |
//! | `a_contained_panic_is_indistinguishable_from_a_collapse` | **the defect, pinned.** Both return `vec![]` from the same function, with no channel that separates them |
//! | `census_direct_cavalier_calls` (`#[ignore]`) | the census: which fixtures reach which cavalier assertion, with payload and source location |
//!
//! # Debug vs release — stated, because the answer differs
//!
//! The assertion the R1 asset trips —
//! `pline_view.rs:507`, *"start index should be less than or equal to end
//! index if polyline is open"* — is a **`debug_assert!`**. It fires here and
//! it does not fire in a release build; in release the invariant is skipped
//! and `from_slice_points` proceeds with `start_index > end_index`. So the
//! `catch_unwind` is a debug/test-only net **for that path**, and what
//! release produces instead is unvalidated.
//!
//! Three sites in cavalier 0.7.0 *do* panic in release and are the ones the
//! shipped containment is actually earning its keep against:
//! `shape_algorithms/mod.rs:786` (`unreachable!("loop_count exceeded
//! max_loop_count while stitching slices together")`) and the hard
//! `assert!`s at `pline_view.rs:316` / `:374` / `:438`.
//!
//! **This wave did not build in release** (programme rule 11), so every
//! result here is a debug result and the release behaviour is recorded as
//! `NOT EXERCISED`, never as `PASS`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use std::sync::{Arc, Mutex};

use cavalier_contours::polyline::{PlineCreation, PlineSource, PlineSourceMut, Polyline};
use cavalier_contours::shape_algorithms::Shape;

use common::adversarial2d as adv;
use common::offset_lab::{PLINE_POS_EQUAL_EPS, load_captured, wanaka_capture_path};

use rs_cam_core::geo::P2;
use rs_cam_core::polygon::{Polygon2, offset_polygon};

/// What one direct call into cavalier did.
#[derive(Debug, Clone)]
enum Direct {
    /// Returned `n` output polylines.
    Ok(usize),
    /// Panicked, with the payload and the source location the panic hook saw.
    Panicked { message: String, location: String },
}

impl Direct {
    fn label(&self) -> String {
        match self {
            Self::Ok(n) => format!("ok ({n} plines)"),
            Self::Panicked { message, location } => format!("PANIC @ {location}: {message}"),
        }
    }
    fn is_panic(&self) -> bool {
        matches!(self, Self::Panicked { .. })
    }
}

fn poly_to_plines(poly: &Polygon2) -> Vec<Polyline<f64>> {
    let mut out = Vec::new();
    let mut ext = Polyline::with_capacity(poly.exterior.len(), true);
    for p in &poly.exterior {
        ext.add(p.x, p.y, 0.0);
    }
    let ext = ext.remove_repeat_pos(PLINE_POS_EQUAL_EPS).unwrap_or(ext);
    if ext.vertex_count() >= 3 {
        out.push(ext);
    }
    for hole in &poly.holes {
        let mut h = Polyline::with_capacity(hole.len(), true);
        for p in hole {
            h.add(p.x, p.y, 0.0);
        }
        let h = h.remove_repeat_pos(PLINE_POS_EQUAL_EPS).unwrap_or(h);
        if h.vertex_count() >= 3 {
            out.push(h);
        }
    }
    out
}

/// Call cavalier the way `offset_polygon_inner` does — `Polyline::parallel_offset`
/// with no holes, `Shape::parallel_offset` with them — but **without** the
/// production `catch_unwind`, so the panic can be observed rather than
/// swallowed.
///
/// The default panic hook is replaced for the duration so a reproduced panic
/// does not print a backtrace storm into the test log; the hook is what
/// recovers the source LOCATION, which the production `Err(_payload)` arm
/// discards along with the message.
fn direct_offset(poly: &Polygon2, distance: f64) -> Direct {
    let plines = poly_to_plines(poly);
    if plines.is_empty() {
        return Direct::Ok(0);
    }
    let has_holes = !poly.holes.is_empty();

    let seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let sink = Arc::clone(&seen);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let loc = info.location().map_or_else(
            || "<unknown>".to_owned(),
            |l| format!("{}:{}", l.file(), l.line()),
        );
        if let Ok(mut slot) = sink.lock() {
            *slot = Some(loc);
        }
    }));

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || -> usize {
        if has_holes {
            let shape = Shape::from_plines(plines);
            let result = shape.parallel_offset(distance, Default::default());
            result.ccw_plines.len() + result.cw_plines.len()
        } else {
            plines
                .first()
                .map(|p| p.parallel_offset(distance).len())
                .unwrap_or(0)
        }
    }));

    std::panic::set_hook(previous);

    match outcome {
        Ok(n) => Direct::Ok(n),
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".to_owned());
            let location = seen
                .lock()
                .ok()
                .and_then(|s| s.clone())
                .unwrap_or_else(|| "<hook did not fire>".to_owned());
            Direct::Panicked { message, location }
        }
    }
}

// ---------------------------------------------------------------------------
// 1. The captured asset is still the input class it was captured as
// ---------------------------------------------------------------------------

/// `test_data/cavalier_panic_polygon_r1.json` — the WANAKA Back Rough terrain
/// slice, captured live from the optimizer grid search on 2026-06-10.
///
/// This asserts only what the asset IS, not what cavalier does with it. A
/// fixture that has silently changed shape cannot evidence a defect, and the
/// R1 sentry already pins the "does not panic through `offset_polygon`" bar.
#[test]
fn the_captured_panic_asset_still_reaches_cavalier_unrepaired() {
    let (poly, distance) = load_captured(&wanaka_capture_path()).expect("R1 capture asset");
    assert_eq!(poly.exterior.len(), 86, "capture asset changed shape");
    assert_eq!(poly.holes.len(), 13, "capture asset changed shape");
    assert!(
        (distance - 5.53).abs() < 1e-6,
        "capture distance changed: {distance}"
    );
    // The holes are what route it into `Shape::parallel_offset` rather than
    // `Polyline::parallel_offset` — i.e. into the stitching code that trips.
    assert!(
        !poly.holes.is_empty(),
        "the Shape path is only taken when holes are present"
    );
    // R1.5 repairs self-intersecting input BEFORE cavalier sees it. Whether
    // this asset is repaired first decides which call actually runs, so it is
    // recorded rather than assumed.
    println!(
        "R1 asset: 86 verts, 13 holes, distance {distance}, \
         self-intersecting = {}",
        poly.has_self_intersection()
    );
    let direct = direct_offset(&poly, distance);
    println!("direct cavalier call on the R1 asset: {}", direct.label());
}

// ---------------------------------------------------------------------------
// 2. The containment holds across the whole hostile library
// ---------------------------------------------------------------------------

/// R1 contained a panic reproduced from ONE captured polygon. This walks the
/// entire R2 fixture library — 22 shapes across eleven hostile classes,
/// including deliberately invalid contours — through the production
/// `offset_polygon` at several distances, and fails if any of them escapes as
/// a panic.
///
/// Distances are chosen relative to each fixture's authored tool: a
/// tool-radius compensation, a stepover-sized inward step, and an outward
/// step, which are the three the 2D families actually ask for.
#[test]
fn no_hostile_input_escapes_the_offset_chokepoint_as_a_panic() {
    let mut escapes = Vec::new();
    let mut collapsed = Vec::new();
    for f in adv::fixtures() {
        let r = f.tool_d * 0.5;
        for distance in [r, f.tool_d * 0.4, -r, 0.05] {
            for (i, poly) in f.polys.iter().enumerate() {
                let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    offset_polygon(poly, distance)
                }));
                match out {
                    Ok(v) => {
                        if v.is_empty() {
                            collapsed.push(format!("{}[{i}] @ {distance:+.3}", f.name));
                        }
                    }
                    Err(_) => escapes.push(format!(
                        "{}[{i}] @ {distance:+.3} PANICKED THROUGH offset_polygon",
                        f.name
                    )),
                }
            }
        }
    }
    println!(
        "{} (polygon, distance) pairs collapsed to an empty Vec:\n  {}",
        collapsed.len(),
        collapsed.join("\n  ")
    );
    assert!(
        escapes.is_empty(),
        "offset_polygon is the single chokepoint; nothing may escape it:\n{}",
        escapes.join("\n")
    );
}

// ---------------------------------------------------------------------------
// 3. The defect, pinned
// ---------------------------------------------------------------------------

/// **The R2-H2 defect, as an assertion.**
///
/// Three structurally different events return the identical value from the
/// identical function:
///
/// 1. a ring below the `< 3`-vertex guard (`polygon.rs:377`);
/// 2. a genuine geometric collapse (the arm is thinner than twice the offset);
/// 3. a contained `cavalier_contours` panic (`polygon.rs:293-303`).
///
/// All three are `Vec::new()`. `offset_polygon` returns `Vec<Polygon2>`, so
/// there is no channel that could carry the difference even if a caller
/// wanted it — and no caller asks, because none can.
///
/// This test passes **today**, describing the current contract. It is written
/// so that it will FAIL the moment a typed failure channel is introduced,
/// which is the point: whoever implements Checkpoint C's ruling has to come
/// here and state the new contract deliberately.
#[test]
fn a_contained_panic_is_indistinguishable_from_a_collapse() {
    // (1) below the vertex guard
    let two = adv::two_vertex(60.0);
    let from_guard = offset_polygon(&two, 1.0);

    // (2) a genuine collapse: a 3 mm-wide bridge offset inward by 5 mm
    let slot = adv::thin_slot(40.0, 3.0, 30.0);
    // Isolate the bridge so the offset really does have nothing left.
    let bridge_only = Polygon2::rectangle(0.0, 0.0, 30.0, 3.0);
    let from_collapse = offset_polygon(&bridge_only, 5.0);
    assert!(
        slot.exterior.len() == 12,
        "the thin-slot fixture is a 12-vertex dumbbell"
    );

    // (3) the captured panic asset, which the containment maps to empty.
    let (captured, distance) = load_captured(&wanaka_capture_path()).expect("R1 capture asset");
    let from_capture = offset_polygon(&captured, distance);

    assert!(
        from_guard.is_empty(),
        "a two-vertex ring must not produce an offset"
    );
    assert!(
        from_collapse.is_empty(),
        "a 3 mm bridge offset inward 5 mm must collapse"
    );
    println!(
        "guard: {} polys | collapse: {} polys | captured R1 asset: {} polys",
        from_guard.len(),
        from_collapse.len(),
        from_capture.len()
    );
    // The contract, stated as an assertion: the value carries no information
    // about WHICH of the three happened.
    assert_eq!(
        from_guard.len(),
        from_collapse.len(),
        "the vertex guard and a real collapse are the same observable value — \
         if this ever fails, a typed failure channel has landed and \
         CAVALIER_SHAPE_FAILURE.md's §6 contract needs re-ruling"
    );
}

// ---------------------------------------------------------------------------
// 4. The census
// ---------------------------------------------------------------------------

/// Which fixtures reach which cavalier assertion, with payload and source
/// location — the table `CAVALIER_SHAPE_FAILURE.md` §3 is generated from.
///
/// `#[ignore]` because it deliberately installs and removes a panic hook
/// around every call, which is process-global: run it single-threaded, on its
/// own.
///
/// ```text
/// cargo test -p rs_cam_core --test cavalier_shape_failure_r2 -- \
///     --ignored --nocapture --test-threads=1 census_direct_cavalier_calls
/// ```
#[test]
#[ignore = "installs a process-global panic hook per call; run single-threaded"]
fn census_direct_cavalier_calls() {
    let mut table = String::from("| fixture | distance | direct cavalier call |\n|---|---|---|\n");
    let mut panics = 0usize;

    if let Some((poly, distance)) = load_captured(&wanaka_capture_path()) {
        let d = direct_offset(&poly, distance);
        if d.is_panic() {
            panics += 1;
        }
        table.push_str(&format!(
            "| CAPTURED wanaka-slice | {distance:+.3} | {} |\n",
            d.label()
        ));
    }

    for f in adv::fixtures() {
        let r = f.tool_d * 0.5;
        for distance in [r, f.tool_d * 0.4, -r, 0.05] {
            for (i, poly) in f.polys.iter().enumerate() {
                // Mirror R1.5: production repairs self-intersection before
                // cavalier is ever called, so a census that skips it is
                // describing a call production never makes.
                let pieces = if poly.has_self_intersection() {
                    poly.repaired()
                } else {
                    vec![poly.clone()]
                };
                for piece in &pieces {
                    let d = direct_offset(piece, distance);
                    if d.is_panic() {
                        panics += 1;
                        table.push_str(&format!(
                            "| {}[{i}] | {distance:+.3} | {} |\n",
                            f.name,
                            d.label()
                        ));
                    }
                }
            }
        }
    }

    table.push_str(&format!("\n**{panics} panicking calls.**\n"));
    let path = adv::out_dir().join("cavalier_panic_census.md");
    let _ = std::fs::write(&path, &table);
    println!("{table}\ncensus: {}", path.display());
}

/// A hand-built input for the class the R1 asset represents: many holes
/// growing into each other under a large inward offset, so the stitching code
/// has to close rings across slices that have already been consumed.
///
/// Reported, not asserted: whether it trips the same assertion is a fact about
/// cavalier 0.7.0, and pinning a *library* panic as a test expectation would
/// make a dependency upgrade look like a regression.
#[test]
fn synthetic_many_hole_shape_offset_is_reported_not_asserted() {
    let mut poly = Polygon2::rectangle(0.0, 0.0, 100.0, 100.0);
    // 16 holes on a 4×4 grid, each large enough that a 6 mm inward offset
    // merges every neighbour pair at once.
    for r in 1..=4 {
        for c in 1..=4 {
            let (cx, cy) = (20.0 * c as f64, 20.0 * r as f64);
            let ring: Vec<P2> = (0..48)
                .map(|k| {
                    let t = -std::f64::consts::TAU * k as f64 / 48.0;
                    P2::new(cx + 7.0 * t.cos(), cy + 7.0 * t.sin())
                })
                .collect();
            poly.holes.push(ring);
        }
    }
    for distance in [1.0, 3.0, 6.0, 6.5, 7.0, 12.0] {
        let direct = direct_offset(&poly, distance);
        let contained = offset_polygon(&poly, distance);
        println!(
            "16-hole grid @ {distance:+.2} mm — direct: {} | offset_polygon: {} polys",
            direct.label(),
            contained.len()
        );
    }
    // The only bar: the production entry point never panics.
    for distance in [1.0, 3.0, 6.0, 6.5, 7.0, 12.0] {
        let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            offset_polygon(&poly, distance)
        }));
        assert!(
            out.is_ok(),
            "offset_polygon must contain every cavalier failure ({distance} mm)"
        );
    }
}
