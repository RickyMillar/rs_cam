//! P22-0c-1 sentry (`planning/feeds_contract_2026-11-03/phases/P22-0c_DESIGN.md`
//! sections 2-4, `P22-0c-1.md`): the tool owns its parts.
//!
//! 1. The zone table of every tool type, built through the one door
//!    (`build_cutter`), equals the table the stated facts give: flutes,
//!    core (only for a non-centre-cutting tool), flute top, shank (only
//!    when the stickout is longer than the flutes).
//! 2. Each view of the body equals that table: the flute length, the
//!    envelope radius, the shank, the holder face, the core limits, and
//!    the old `ToolAssembly` view of the mounted body.
//! 3. Memo arm: two tools that differ only in `centre_cutting` get two
//!    simulation-prefix keys (D1); two equal tools share one.
//! 4. P22-0c-2 search guard (design section 4): no source file under
//!    `src/` halves the core diameter outside `tool/body.rs`. Every reader
//!    takes the core radius from the body (`ToolBody::core`). Comments do
//!    not count; the anchor is the one site in `tool/body.rs`.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::sim_prefix::{SimMemo, SimPrefixCache};
use rs_cam_core::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, run_simulation_memoized,
};
use rs_cam_core::compute::tool_config::{
    CentreCutting, NonCentreCuttingGeometry, ToolConfig, ToolFact, ToolId, ToolType,
};
use rs_cam_core::dexel_stock::StockCutDirection;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::head::{HeadBandKind, HeadFit};
use rs_cam_core::stock::collision::AssemblySegment;
use rs_cam_core::tool::body::{
    AxialSpan, ContactKind, ContactRadius, ContactZone, CoreZone, CuttingKind, CuttingZone,
    SpanTop, Zone, core_of, cut_table, envelope_table,
};

#[path = "common/source_scan.rs"]
mod source_scan;
use rs_cam_core::tool::{MillingCutter, ToolDefinition};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

/// The stated facts of one case.
struct Case {
    label: &'static str,
    tool_type: ToolType,
    diameter: f64,
    flute_mm: f64,
    shank_mm: f64,
    stickout_mm: f64,
    /// The radius the flute top reaches: the cutter envelope.
    envelope_mm: f64,
    core: Option<NonCentreCuttingGeometry>,
}

fn config(case: &Case) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), case.tool_type);
    tool.diameter = case.diameter;
    tool.set_cutting_length(ToolFact::Stated(case.flute_mm));
    tool.set_shank_diameter(ToolFact::Stated(case.shank_mm));
    tool.set_stickout(ToolFact::Stated(case.stickout_mm));
    tool.set_flute_count(ToolFact::Stated(2));
    tool.set_centre_cutting(ToolFact::Stated(match case.core {
        Some(geometry) => CentreCutting::No(geometry),
        None => CentreCutting::Yes,
    }));
    match case.tool_type {
        ToolType::EndMill | ToolType::BallNose => {}
        ToolType::BullNose => tool.corner_radius = 1.0,
        ToolType::VBit => {
            tool.included_angle = 90.0;
            tool.set_v_tip_diameter(ToolFact::Stated(0.0));
        }
        ToolType::TaperedBallNose => {
            tool.set_taper_half_angle(ToolFact::Stated(7.0));
            tool.shaft_diameter = 6.0;
        }
    }
    tool
}

/// The zone table the stated facts give (design section 3).
fn expected_zones(case: &Case) -> Vec<Zone> {
    let l = case.flute_mm;
    let mut zones = vec![Zone::Cutting(CuttingZone {
        span: AxialSpan {
            bottom_mm: 0.0,
            top: SpanTop::At(l),
        },
        inner_mm: case.core.map_or(0.0, |c| c.centre_core_diameter_mm / 2.0),
        kind: CuttingKind::Flutes,
    })];
    if let Some(core) = case.core {
        zones.push(Zone::Contact(ContactZone {
            span: AxialSpan {
                bottom_mm: core.blade_protrusion_mm,
                top: SpanTop::At(l),
            },
            inner_mm: 0.0,
            outer: ContactRadius::Mm(core.centre_core_diameter_mm / 2.0),
            kind: ContactKind::Core,
            name: None,
        }));
    }
    zones.push(Zone::Contact(ContactZone {
        span: AxialSpan {
            bottom_mm: l,
            top: SpanTop::At(l),
        },
        inner_mm: 0.0,
        outer: ContactRadius::Mm(case.envelope_mm),
        kind: ContactKind::FluteTop,
        name: None,
    }));
    if case.stickout_mm > l {
        zones.push(Zone::Contact(ContactZone {
            span: AxialSpan {
                bottom_mm: l,
                top: SpanTop::At(case.stickout_mm),
            },
            inner_mm: 0.0,
            outer: ContactRadius::Mm(case.shank_mm / 2.0),
            kind: ContactKind::Shank,
            name: None,
        }));
    }
    zones
}

fn wanaka_core() -> NonCentreCuttingGeometry {
    NonCentreCuttingGeometry {
        centre_core_diameter_mm: 8.0,
        blade_protrusion_mm: 1.0,
    }
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            label: "flat",
            tool_type: ToolType::EndMill,
            diameter: 6.0,
            flute_mm: 20.0,
            shank_mm: 6.0,
            stickout_mm: 35.0,
            envelope_mm: 3.0,
            core: None,
        },
        Case {
            label: "ball",
            tool_type: ToolType::BallNose,
            diameter: 3.0,
            flute_mm: 12.0,
            shank_mm: 6.0,
            stickout_mm: 30.0,
            envelope_mm: 1.5,
            core: None,
        },
        Case {
            label: "bull",
            tool_type: ToolType::BullNose,
            diameter: 10.0,
            flute_mm: 25.0,
            shank_mm: 10.0,
            stickout_mm: 40.0,
            envelope_mm: 5.0,
            core: None,
        },
        Case {
            label: "V-bit",
            tool_type: ToolType::VBit,
            diameter: 12.0,
            flute_mm: 15.0,
            shank_mm: 6.35,
            stickout_mm: 30.0,
            envelope_mm: 6.0,
            core: None,
        },
        Case {
            label: "tapered ball (envelope = shaft / 2)",
            tool_type: ToolType::TaperedBallNose,
            diameter: 1.0,
            flute_mm: 30.5,
            shank_mm: 6.0,
            stickout_mm: 45.0,
            envelope_mm: 3.0,
            core: None,
        },
        Case {
            label: "non-centre-cutting flat (wanaka tool 6)",
            tool_type: ToolType::EndMill,
            diameter: 18.0,
            flute_mm: 30.0,
            shank_mm: 12.0,
            stickout_mm: 45.0,
            envelope_mm: 9.0,
            core: Some(wanaka_core()),
        },
        Case {
            label: "stickout shorter than the flute (FC0): no shank zone",
            tool_type: ToolType::EndMill,
            diameter: 10.0,
            flute_mm: 25.0,
            shank_mm: 6.35,
            stickout_mm: 20.0,
            envelope_mm: 5.0,
            core: None,
        },
    ]
}

#[test]
fn one_body_for_every_reader_p22_0c() {
    for case in cases() {
        let label = case.label;
        let tool = config(&case);
        let def = build_cutter(&tool).expect(label);
        let body = def.body();

        // 1. The zone table.
        assert_eq!(body.zones(), expected_zones(&case), "{label}: zone table");

        // 2. Every view reads the table.
        assert_eq!(body.flute_length_mm(), case.flute_mm, "{label}");
        assert_eq!(def.length(), case.flute_mm, "{label}: the cutter agrees");
        assert_eq!(body.envelope_radius_mm(), case.envelope_mm, "{label}");
        assert_eq!(def.envelope_radius_mm(), case.envelope_mm, "{label}");
        assert_eq!(def.shank_diameter(), case.shank_mm, "{label}");
        assert_eq!(def.stickout(), case.stickout_mm, "{label}");
        assert_eq!(body.holder_face_mm(), case.stickout_mm, "{label}");
        assert_eq!(
            def.exposed_shank_length(),
            (case.stickout_mm - case.flute_mm).max(0.0),
            "{label}"
        );
        // P22-0c-2: the core view reads the table; the record, the view of
        // a cutter (`core_of`) and the body agree.
        let core_zone = body.zones().into_iter().find_map(|zone| match zone {
            Zone::Contact(ContactZone {
                kind: ContactKind::Core,
                span,
                outer: ContactRadius::Mm(radius_mm),
                ..
            }) => Some(CoreZone {
                radius_mm,
                protrusion_mm: span.bottom_mm,
            }),
            Zone::Contact(_) | Zone::Cutting(_) => None,
        });
        assert_eq!(body.core(), core_zone, "{label}: core view");
        assert_eq!(
            body.core(),
            tool.non_centre_cutting_geometry()
                .map(CoreZone::from_geometry),
            "{label}: core view against the record"
        );
        assert_eq!(core_of(&def), body.core(), "{label}: the cutter view");
        // The cut table carries the ring of the core; the envelope table
        // the full disc.
        let ring = cut_table(&def, 256).blade_ring_core_sq();
        assert_eq!(
            ring,
            body.core()
                .map(|core| (core.radius_mm * core.radius_mm)
                    .min(case.envelope_mm * case.envelope_mm)),
            "{label}: cut table ring"
        );
        assert_eq!(
            envelope_table(&def, 256).blade_ring_core_sq(),
            None,
            "{label}: envelope table"
        );

        // The mounted body: the tool zones, then the head bands.
        let fit = HeadFit::default();
        let mounted = def.to_assembly(&fit);
        let zones = mounted.zones();
        let tool_zones = body.zones();
        assert_eq!(&zones[..tool_zones.len()], &tool_zones[..], "{label}");
        let head = fit.head.bands(case.stickout_mm, fit.shoe);
        assert_eq!(mounted.head(), &head[..], "{label}");
        assert_eq!(zones.len(), tool_zones.len() + head.len(), "{label}");
        for (zone, band) in zones[tool_zones.len()..].iter().zip(&head) {
            let want = Zone::Contact(ContactZone {
                span: AxialSpan {
                    bottom_mm: band.offset_mm,
                    top: SpanTop::Open,
                },
                inner_mm: 0.0,
                outer: band
                    .radius_mm
                    .map_or(ContactRadius::Shelf, ContactRadius::Mm),
                kind: match band.kind {
                    HeadBandKind::Holder => ContactKind::Holder,
                    HeadBandKind::Shoe => ContactKind::Shoe,
                },
                name: Some(band.name.clone()),
            });
            assert_eq!(zone, &want, "{label}: head zone");
        }

        // The old assembly view: the same facts, and its bands are the
        // shank zone then the head bands with a width.
        let asm = mounted.assembly();
        assert_eq!(asm.cutter_radius, def.radius(), "{label}");
        assert_eq!(asm.cutter_length, case.flute_mm, "{label}");
        assert_eq!(asm.shank_diameter, case.shank_mm, "{label}");
        assert_eq!(asm.stickout(), case.stickout_mm, "{label}");
        let bands: Vec<(AssemblySegment, f64, f64)> = asm
            .segments()
            .iter()
            .map(|b| (b.segment, b.z_offset, b.radius))
            .collect();
        let mut want: Vec<(AssemblySegment, f64, f64)> = Vec::new();
        for zone in &zones {
            let Zone::Contact(contact) = zone else {
                continue;
            };
            let ContactRadius::Mm(radius) = contact.outer else {
                continue;
            };
            let segment = match contact.kind {
                ContactKind::Core | ContactKind::FluteTop => continue,
                ContactKind::Shank => AssemblySegment::Shank,
                ContactKind::Holder => AssemblySegment::Holder,
                ContactKind::Shoe => AssemblySegment::Shoe,
            };
            want.push((segment, contact.span.bottom_mm, radius));
        }
        assert_eq!(bands, want, "{label}: the assembly bands");
    }
}

// ── Memo arm (D1) ────────────────────────────────────────────────────────

fn memo_tool(core: Option<NonCentreCuttingGeometry>) -> ToolDefinition {
    build_cutter(&config(&Case {
        label: "memo",
        tool_type: ToolType::EndMill,
        diameter: 18.0,
        flute_mm: 30.0,
        shank_mm: 12.0,
        stickout_mm: 45.0,
        envelope_mm: 9.0,
        core,
    }))
    .unwrap()
}

fn pass(index: usize) -> Arc<AnnotatedToolpath> {
    let y = 10.0 + 12.0 * index as f64;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(-15.0, y, 5.0));
    tp.feed_to(P3::new(-15.0, y, -1.0), 600.0);
    tp.feed_to(P3::new(75.0, y, -1.0), 900.0);
    tp.rapid_to(P3::new(75.0, y, 5.0));
    Arc::new(AnnotatedToolpath::new(tp))
}

fn request(
    chain: &[Arc<AnnotatedToolpath>],
    tool: &Arc<ToolDefinition>,
    count: usize,
) -> SimulationRequest {
    SimulationRequest {
        groups: vec![SimGroupEntry {
            toolpaths: (0..count)
                .map(|i| SimToolpathEntry {
                    head: HeadFit::default(),
                    id: ToolpathId::from_raw(i + 1),
                    name: format!("Pass{i}"),
                    annotated: Arc::clone(&chain[i]),
                    tool: Arc::clone(tool),
                    tool_id: ToolId::from_raw(0),
                    flute_count: 2,
                    tool_summary: "18mm Flat".to_owned(),
                    semantic_trace: None,
                    spindle_rpm: None,
                    metrics_not_applicable: false,
                    drill_op: None,
                    operation_config_hash: i as u64,
                })
                .collect(),
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            phantom_prior_stock: None,
            stock_changes: Vec::new(),
            obstacles: Ok(Default::default()),
        }],
        stock_bbox: BoundingBox3 {
            min: P3::new(0.0, 0.0, -6.0),
            max: P3::new(60.0, 40.0, 0.0),
        },
        stock_top_z: 0.0,
        resolution: 0.5,
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: None,
        kinematics: None,
        display_stride: 1,
    }
}

/// The hits of a three-pass run after a stored two-pass run of `first`.
fn hits_after(first: &Arc<ToolDefinition>, second: &Arc<ToolDefinition>) -> u64 {
    // The memo keys a pass by its `Arc`: both runs share one chain.
    let chain: Vec<Arc<AnnotatedToolpath>> = (0..3).map(pass).collect();
    let mut cache = SimPrefixCache::new();
    let cancel = AtomicBool::new(false);
    run_simulation_memoized(
        &request(&chain, first, 2),
        &cancel,
        |_phase| {},
        Some(SimMemo {
            cache: &mut cache,
            store: true,
        }),
    )
    .expect("simulation");
    run_simulation_memoized(
        &request(&chain, second, 3),
        &cancel,
        |_phase| {},
        Some(SimMemo {
            cache: &mut cache,
            store: true,
        }),
    )
    .expect("simulation");
    cache.stats().hits
}

#[test]
fn a_centre_cutting_change_gives_a_new_prefix_key_p22_0c() {
    let centre = Arc::new(memo_tool(None));
    let centre_again = Arc::new(memo_tool(None));
    let non_centre = Arc::new(memo_tool(Some(wanaka_core())));
    assert_eq!(
        hits_after(&centre, &centre_again),
        1,
        "control: two equal tools share the prefix key"
    );
    assert_eq!(
        hits_after(&centre, &non_centre),
        0,
        "a tool that differs only in centre_cutting must not reuse the stock"
    );
    assert_eq!(
        hits_after(&non_centre, &centre),
        0,
        "the same, in the other order"
    );
}

/// The places in `text` (comments removed, white space removed) where a
/// core diameter is halved: `core_diameter_mm / 2`, `core_diameter_mm *
/// 0.5`, or `0.5 * <path>core_diameter_mm`.
fn halved_core_diameters(text: &str) -> usize {
    let flat: String = source_scan::strip_comments(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let needle = "core_diameter_mm";
    flat.match_indices(needle)
        .filter(|(at, _)| {
            let after = &flat[at + needle.len()..];
            let before = &flat[..*at];
            // The path in front of the name: `geometry.centre_`, `limits.`.
            let path_start = before
                .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
                .map_or(0, |i| i + 1);
            after.starts_with("/2")
                || after.starts_with("*0.5")
                || before[..path_start].ends_with("0.5*")
        })
        .count()
}

/// P22-0c-2 search guard: the core radius has ONE definition,
/// `CoreZone::from_geometry` in `tool/body.rs`.
#[test]
fn no_reader_halves_the_core_diameter_outside_the_body_p22_0c() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let body = src.join("tool").join("body.rs");
    let anchor = halved_core_diameters(&std::fs::read_to_string(&body).unwrap());
    assert_eq!(anchor, 1, "non-vacuity: tool/body.rs holds the one site");
    assert_eq!(
        halved_core_diameters("// x.centre_core_diameter_mm / 2.0\nlet a = 1;"),
        0,
        "a comment does not count"
    );
    assert_eq!(
        halved_core_diameters("let r = 0.5 * limits.centre_core_diameter_mm;"),
        1,
        "the 0.5 * form counts"
    );
    let offenders: Vec<String> = source_scan::rust_files(&src)
        .into_iter()
        .filter(|path| *path != body)
        .filter(|path| halved_core_diameters(&std::fs::read_to_string(path).unwrap()) > 0)
        .map(|path| path.display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "these files halve the core diameter; read ToolBody::core instead: {offenders:?}"
    );
}
