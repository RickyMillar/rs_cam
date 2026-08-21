//! Sentries for the 6-view composite renderer's camera convention.
//!
//! Five defects were catalogued in `planning/airrun_2026-08-19/RUN_LOG.md`
//! ("Renderer defects the frame bug was sitting behind"). Four of them are
//! properties of the *image*, so they are pinned here by reading pixels back
//! rather than by trusting the source:
//!
//! 1. the composite dropped `StockConfig::origin` — every panel re-centred on
//!    the mesh's own centroid, so a mesh in the wrong frame looked right;
//! 2. the two polar panels were mirror-inconsistent;
//! 3. rim-wall "vertical striping" was pixel-grid aliasing, not material;
//! 5. every panel auto-fitted its own extents, so no two scales agreed.
//!
//! Defect 4 (no captions, and the internal view names named the wrong corner)
//! is pinned by `every_panel_carries_a_caption` plus the label strings in
//! `composite_panel_layout`.
//!
//! `reference_composites_for_visual_review` writes PNGs to
//! `target/render_checks/` — that is the artifact to open and look at.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::fingerprint::{
    CompositePanel, composite_panel_layout, render_mesh_composite, render_stock_composite,
    render_stock_composite_in_frame,
};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::stock_mesh::StockMesh;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::Toolpath;

const W: u32 = 900;
const H: u32 = 600;

fn out_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/render_checks");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn save(name: &str, pixels: &[u8], w: u32, h: u32) {
    let img = image::RgbaImage::from_raw(w, h, pixels.to_vec()).unwrap();
    img.save(out_dir().join(name)).unwrap();
}

fn panel(panels: &[CompositePanel], label: &str) -> CompositePanel {
    *panels
        .iter()
        .find(|p| p.label == label)
        .unwrap_or_else(|| panic!("no panel labelled {label}"))
}

/// Panel-local bounding box of the pixels that came from the mesh.
///
/// Material is identified by **chroma**, not brightness: every pixel the
/// overlay draws — background 42, panel separators 60, caption plates 18,
/// caption text, footer text — is achromatic or near enough, while the
/// height-gradient mesh colours are strongly saturated. That keeps the
/// measurement independent of where captions happen to sit.
fn material_bbox(
    pixels: &[u8],
    w: usize,
    p: &CompositePanel,
) -> Option<(usize, usize, usize, usize)> {
    let mut x0 = usize::MAX;
    let mut y0 = usize::MAX;
    let mut x1 = 0usize;
    let mut y1 = 0usize;
    let mut found = false;
    for py in 0..p.height {
        for px in 0..p.width {
            let i = ((p.y + py) * w + p.x + px) * 4;
            let r = i32::from(pixels[i]);
            let g = i32::from(pixels[i + 1]);
            let b = i32::from(pixels[i + 2]);
            if r.max(g).max(b) - r.min(g).min(b) < 25 {
                continue;
            }
            found = true;
            x0 = x0.min(px);
            y0 = y0.min(py);
            x1 = x1.max(px);
            y1 = y1.max(py);
        }
    }
    if found { Some((x0, y0, x1, y1)) } else { None }
}

/// A 40 mm cube of stock sitting in the low-X / low-Y corner of the world.
fn corner_stock() -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(40.0, 40.0, 10.0),
    };
    TriDexelStock::from_bounds(&bbox, 1.0)
}

/// A camera frame five times the stock, so the stock's *position* in the
/// frame is a visible, measurable thing rather than something auto-fit away.
fn wide_frame() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(200.0, 200.0, 20.0),
    }
}

/// Defects 1 and 5. The stock occupies the low-X / low-Y corner of the camera
/// frame, so in the Top panel — +X right, +Y up — it must land left of centre
/// and *below* centre, at a fraction of the panel's size.
///
/// The pre-fix renderer centred every panel on the mesh centroid and auto-fit
/// each panel to its own extents, so this stock filled the panel dead centre
/// no matter what world frame it was in. That is precisely what let a setup
/// drawn in the wrong frame look correct.
#[test]
fn composite_anchors_to_the_world_frame_not_the_mesh_centroid() {
    let stock = corner_stock();
    let frame = wide_frame();
    let pixels = render_stock_composite_in_frame(&stock, &frame, W, H);
    let panels = composite_panel_layout(W, H);
    let top = panel(&panels, "TOP");

    let (x0, y0, x1, _y1) =
        material_bbox(&pixels, W as usize, &top).expect("top panel drew some material");

    let cx = top.width / 2;
    let cy = top.height / 2;
    assert!(
        x1 < cx,
        "stock at low X must render left of the panel centre: x1={x1} centre={cx}"
    );
    assert!(
        y0 > cy,
        "stock at low Y must render below the panel centre (+Y is up): y0={y0} centre={cy}"
    );

    // 40 mm of a 200 mm frame: the stock must occupy roughly a fifth of the
    // panel, not fill it. A per-panel auto-fit would give ~1.0 here.
    let occupancy = (x1 - x0) as f64 / top.width as f64;
    assert!(
        (0.08..0.30).contains(&occupancy),
        "stock should fill about a fifth of the frame, got {occupancy:.3}"
    );
}

/// Defect 2. The corrected camera is `r = normalize(Ẑ × n)`, `u = n × r`, and
/// both polar panels take azimuth π — so world +X points right in Top *and*
/// Bottom, and only the elevation sign flips the up axis.
///
/// That convention is not a free choice: `compute::transform::FaceUp::Bottom`
/// is "Flip 180 deg on X axis", implemented as `(x, D−y, H−z)`. X survives,
/// Y inverts. The Bottom panel therefore shows the part as it lies after the
/// flip this CAM system actually models, and the pair can be read
/// edge-for-edge. If `FaceUp::Bottom` ever changes, this test is the thing
/// that should fail first.
#[test]
fn top_and_bottom_share_x_and_flip_only_y() {
    let stock = corner_stock();
    let frame = wide_frame();
    let pixels = render_stock_composite_in_frame(&stock, &frame, W, H);
    let panels = composite_panel_layout(W, H);
    let top = panel(&panels, "TOP");
    let bottom = panel(&panels, "BOTTOM");

    let (tx0, ty0, tx1, ty1) = material_bbox(&pixels, W as usize, &top).unwrap();
    let (bx0, by0, bx1, by1) = material_bbox(&pixels, W as usize, &bottom).unwrap();

    let tol = 3i64;
    let d = |a: usize, b: usize| (a as i64 - b as i64).abs();

    assert!(
        d(tx0, bx0) <= tol && d(tx1, bx1) <= tol,
        "X must NOT flip between Top and Bottom: top x [{tx0},{tx1}] bottom x [{bx0},{bx1}]"
    );

    // Y mirrors about the panel centre.
    let last = top.height as i64 - 1;
    assert!(
        (last - ty1 as i64 - by0 as i64).abs() <= tol,
        "Bottom's top edge must mirror Top's bottom edge: ty1={ty1} by0={by0} h={}",
        top.height
    );
    assert!(
        (last - ty0 as i64 - by1 as i64).abs() <= tol,
        "Bottom's bottom edge must mirror Top's top edge: ty0={ty0} by1={by1} h={}",
        top.height
    );
    assert!(
        d(ty1 - ty0, by1 - by0) <= tol,
        "the flip must not change the footprint's size"
    );
}

/// Defect 5. One mm/px for the whole composite. Two isometric panels whose
/// azimuths differ by 180° see an axis-aligned box at identical projected
/// extents, so any disagreement between them is a scale disagreement.
#[test]
fn all_panels_share_one_scale() {
    let stock = corner_stock();
    let frame = wide_frame();
    let pixels = render_stock_composite_in_frame(&stock, &frame, W, H);
    let panels = composite_panel_layout(W, H);

    let a = material_bbox(&pixels, W as usize, &panel(&panels, "REAR-RIGHT")).unwrap();
    let b = material_bbox(&pixels, W as usize, &panel(&panels, "FRONT-LEFT")).unwrap();

    let aw = a.2 - a.0;
    let ah = a.3 - a.1;
    let bw = b.2 - b.0;
    let bh = b.3 - b.1;
    assert!(
        (aw as i64 - bw as i64).abs() <= 3 && (ah as i64 - bh as i64).abs() <= 3,
        "opposite iso panels must render the same box at the same size: \
         rear-right {aw}x{ah}, front-left {bw}x{bh}"
    );

    // Top and Bottom see the same square footprint, so they must agree too.
    let t = material_bbox(&pixels, W as usize, &panel(&panels, "TOP")).unwrap();
    let bo = material_bbox(&pixels, W as usize, &panel(&panels, "BOTTOM")).unwrap();
    assert!(
        ((t.2 - t.0) as i64 - (bo.2 - bo.0) as i64).abs() <= 3,
        "Top and Bottom must share a scale"
    );
}

/// Defect 3. `dexel_stock_to_mesh` emits one wall quad per ~0.4 mm dexel
/// column while a panel resolves ~0.9 mm/px, so the wall quads used to beat
/// against the pixel grid and produce "vertical striping" that read as
/// standing material. The measured stripe pitch tracked the render size, not
/// the geometry, which is the signature of aliasing.
///
/// The fix is to supersample and box-downsample, i.e. to report the *average*
/// over each pixel's footprint. This test pins the observable consequence on
/// a single flat-shaded triangle: with one triangle colour and one background
/// colour, **any** third colour in the image can only have come from partial
/// pixel coverage. A hard-edged rasterizer produces exactly zero of them.
#[test]
fn silhouette_edges_are_antialiased() {
    // Deliberately slanted edges: an axis-aligned edge can land exactly on a
    // pixel boundary and legitimately produce no partial coverage at all.
    let mesh = StockMesh {
        vertices: vec![0.0, 0.0, 0.0, 100.0, 12.0, 0.0, 28.0, 90.0, 0.0],
        indices: vec![0, 1, 2],
        colors: vec![0.9, 0.3, 0.1, 0.9, 0.3, 0.1, 0.9, 0.3, 0.1],
    };
    let pixels = render_mesh_composite(&mesh, W, H);
    let panels = composite_panel_layout(W, H);
    let top = panel(&panels, "TOP");

    // Skip the caption band and the panel separators; everything else in this
    // panel is either background, the triangle, or a blend of the two.
    let y_start = top.height / 3;
    let mut blends = 0usize;
    let mut solid = 0usize;
    for py in y_start..top.height.saturating_sub(2) {
        for px in 2..top.width.saturating_sub(2) {
            let i = ((top.y + py) * W as usize + top.x + px) * 4;
            let r = i32::from(pixels[i]);
            let g = i32::from(pixels[i + 1]);
            let b = i32::from(pixels[i + 2]);
            let chroma = r.max(g).max(b) - r.min(g).min(b);
            if chroma < 20 {
                continue; // background / separators / captions
            }
            if r > 199 {
                solid += 1; // the triangle's own colour
            } else if r > 52 {
                blends += 1; // partial coverage: only anti-aliasing makes these
            }
        }
    }

    assert!(
        solid > 500,
        "the triangle should cover the panel: {solid} px"
    );
    assert!(
        blends >= 30,
        "silhouette must carry partially-covered pixels; got {blends} \
         (zero means the render is hard-edged again and will invent stripes \
         on dexel wall quads)"
    );
}

/// Defect 4. Nothing used to be drawn on the panels, and the internal names
/// pointed at the wrong corner — "Front-Left" had its eye at (+X, +Y).
#[test]
fn every_panel_carries_a_caption() {
    let stock = corner_stock();
    let pixels = render_stock_composite(&stock, W, H);
    let panels = composite_panel_layout(W, H);
    assert_eq!(panels.len(), 6);

    for p in &panels {
        let mut white = 0usize;
        for py in 0..(p.height / 4) {
            for px in 0..p.width {
                let i = ((p.y + py) * W as usize + p.x + px) * 4;
                if pixels[i] == 255 && pixels[i + 1] == 255 && pixels[i + 2] == 255 {
                    white += 1;
                }
            }
        }
        assert!(
            white > 20,
            "panel {} has no caption drawn ({white} white px)",
            p.label
        );
    }

    // The names must say where the camera is, in machine terms: +X is table
    // right, +Y runs away from the operator, so -Y is the "front" of the job.
    let names: Vec<&str> = panels.iter().map(|p| p.label).collect();
    assert_eq!(
        names,
        vec![
            "REAR-LEFT",
            "TOP",
            "REAR-RIGHT",
            "FRONT-LEFT",
            "BOTTOM",
            "FRONT-RIGHT"
        ]
    );
    assert_eq!(panel(&panels, "TOP").axes, "+X RIGHT +Y UP");
    assert_eq!(panel(&panels, "BOTTOM").axes, "+X RIGHT +Y DOWN");
    assert_eq!(panel(&panels, "REAR-RIGHT").axes, "EYE +X +Y ABOVE");
    assert_eq!(panel(&panels, "FRONT-LEFT").axes, "EYE -X -Y ABOVE");
}

/// Writes the composites to look at. Not a gate — the assertions are only
/// there so a broken render fails loudly instead of writing a broken PNG.
///
/// Artifacts land in `target/render_checks/`:
///
/// * `composite_offset_frame.png` — 40 mm stock in a 200 mm world frame.
///   Read the footer: it names the frame the picture was drawn in.
/// * `composite_pocket.png` — a pocketed block, for judging rim-wall
///   striping. Any regular vertical banding on the walls here is aliasing.
/// * `composite_pocket_1400.png` — the same block at a different size. If
///   banding pitch changes with render size it is the pixel grid, not
///   geometry.
#[test]
fn reference_composites_for_visual_review() {
    let frame = wide_frame();
    let offset = render_stock_composite_in_frame(&corner_stock(), &frame, W, H);
    assert_eq!(offset.len(), (W * H * 4) as usize);
    save("composite_offset_frame.png", &offset, W, H);

    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(120.0, 90.0, 20.0),
    };
    let mut stock = TriDexelStock::from_bounds(&bbox, 0.4);

    // An off-centre pocket, so left/right and front/rear are distinguishable
    // at a glance in every panel.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(20.0, 20.0, 25.0));
    tp.feed_to(P3::new(20.0, 20.0, 12.0), 600.0);
    for ring in 0..6 {
        let inset = f64::from(ring) * 3.0;
        tp.feed_to(P3::new(70.0 - inset, 20.0 + inset, 12.0), 1200.0);
        tp.feed_to(P3::new(70.0 - inset, 55.0 - inset, 12.0), 1200.0);
        tp.feed_to(P3::new(20.0 + inset, 55.0 - inset, 12.0), 1200.0);
        tp.feed_to(P3::new(20.0 + inset, 23.0 + inset, 12.0), 1200.0);
    }
    tp.rapid_to(P3::new(20.0, 20.0, 25.0));

    let cutter = FlatEndmill::new(6.0, 25.0);
    stock.simulate_toolpath(&tp, &cutter, StockCutDirection::FromTop);

    let a = render_stock_composite(&stock, W, H);
    assert_eq!(a.len(), (W * H * 4) as usize);
    save("composite_pocket.png", &a, W, H);

    let (w2, h2) = (1400u32, 933u32);
    let b = render_stock_composite(&stock, w2, h2);
    assert_eq!(b.len(), (w2 * h2 * 4) as usize);
    save("composite_pocket_1400.png", &b, w2, h2);
}
