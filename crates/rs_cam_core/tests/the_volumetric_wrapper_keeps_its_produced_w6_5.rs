//! W6.5 step 0: the 3D Rough side channel stays the same.
//!
//! # Why this exists
//!
//! G-OPGOLDEN (`every_op_keeps_its_toolpath_g_opgolden.rs`) hashes the
//! moves and the spans, and the rest output of a rest detector. It does not
//! hash the two items that the 3D Rough planner puts on `Produced`:
//!
//! - `planner_engagement`: the Stage 4 samples `(cut point, alpha / 2 pi)`
//!   of ContourSpiral, which the feed modulator reads after the dressups;
//! - `area_regions`: the By Area detection, which the viewport overlay and
//!   MCP `inspect_spans` read.
//!
//! The W6.5 volumetric wrapper moves the attach of `Produced` out of the
//! adapter. This file proves that the two items do not change. Lead ruling
//! (W6.5): the digest has its own sentry and its own blessed file; the
//! G-OPGOLDEN format does not change.
//!
//! # The arms
//!
//! The G-OPGOLDEN 3D Rough arms, through the same session door and the
//! same fixtures (`common::op_fixtures`): `fresh`, `rest`,
//! `by_area_helix_spiral` and `ramp_agent`.
//!
//! # Blessing
//!
//! `RS_CAM_BLESS_PRODUCED_W6_5=1` rewrites the blessed file. Re-bless only
//! for an intended change, and say so in the commit body.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test the_volumetric_wrapper_keeps_its_produced_w6_5
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
#![expect(
    clippy::print_stderr,
    reason = "The per-arm diff and the bless notice ARE the record."
)]

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use common::fingerprint::fnv1a_debug;
use common::op_fixtures::{
    CaseSpec, Fixture, Frame, Prior, case_session, finest_decimals, quantize, run_prior,
};
use rayon::prelude::*;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{
    Adaptive3dEntryStyle, ClearingStrategy, RegionOrdering,
};
use rs_cam_core::trace::produced::Produced;
use serde::{Deserialize, Serialize};

/// The environment switch that rewrites the blessed file.
const BLESS_ENV: &str = "RS_CAM_BLESS_PRODUCED_W6_5";

/// The digest of the 3D Rough side channel of one arm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct VolumetricProduced {
    /// `ok`, or the refusal text.
    outcome: String,
    /// The count of the engagement samples.
    engagement_samples: usize,
    /// FNV-1a over the quantized samples, as hex.
    engagement_hash: String,
    /// The count of the By Area regions. 0 when there is no map.
    area_region_count: usize,
    /// FNV-1a over the quantized map, as hex. Empty when there is no map.
    area_regions_hash: String,
}

impl VolumetricProduced {
    fn of(produced: &Produced) -> Self {
        let d = finest_decimals().xyz;
        let q = |v: f64| quantize(v, d);
        let samples: Vec<([i64; 3], i64)> = produced
            .planner_engagement
            .iter()
            .map(|(p, frac)| ([q(p.x), q(p.y), q(p.z)], q(*frac)))
            .collect();
        let (count, area_hash) = produced.area_regions.as_deref().map_or_else(
            || (0, String::new()),
            |map| {
                let regions: Vec<_> = map
                    .regions
                    .iter()
                    .map(|r| {
                        (
                            r.order,
                            r.cell_count,
                            r.bbox_xy.map(q),
                            r.surface_z_range.map(q),
                            r.level_z_range.map(|z| z.map(q)),
                            r.level_count,
                        )
                    })
                    .collect();
                let spec = (
                    q(map.origin_x),
                    q(map.origin_y),
                    q(map.cell_mm),
                    map.rows,
                    map.cols,
                    q(map.top_z),
                );
                (
                    map.regions.len(),
                    format!("{:016x}", fnv1a_debug(&(spec, &map.labels, regions))),
                )
            },
        );
        Self {
            outcome: "ok".to_owned(),
            engagement_samples: samples.len(),
            engagement_hash: format!("{:016x}", fnv1a_debug(&samples)),
            area_region_count: count,
            area_regions_hash: area_hash,
        }
    }

    fn refused(message: &str) -> Self {
        Self {
            outcome: format!("refused: {message}"),
            engagement_samples: 0,
            engagement_hash: String::new(),
            area_region_count: 0,
            area_regions_hash: String::new(),
        }
    }
}

type Tweak = fn(&mut OperationConfig);

fn no_tweak(_: &mut OperationConfig) {}

/// The same switches as the G-OPGOLDEN arm of the same name.
fn by_area_helix_spiral(op: &mut OperationConfig) {
    let OperationConfig::Adaptive3d(cfg) = op else {
        unreachable!("adaptive3d arm on another op")
    };
    cfg.region_ordering = RegionOrdering::ByArea;
    cfg.entry_style = Adaptive3dEntryStyle::Helix;
    cfg.clearing_strategy = ClearingStrategy::ContourSpiral;
}

/// The same switches as the G-OPGOLDEN arm of the same name.
fn ramp_agent(op: &mut OperationConfig) {
    let OperationConfig::Adaptive3d(cfg) = op else {
        unreachable!("adaptive3d arm on another op")
    };
    cfg.entry_style = Adaptive3dEntryStyle::Ramp;
    cfg.clearing_strategy = ClearingStrategy::AgentSearch;
}

/// The arms: name, tweak, rest source.
const ARMS: &[(&str, Tweak, bool)] = &[
    ("fresh", no_tweak, false),
    ("rest", no_tweak, true),
    ("by_area_helix_spiral", by_area_helix_spiral, false),
    ("ramp_agent", ramp_agent, false),
];

fn run(tweak: Tweak, rest: bool) -> VolumetricProduced {
    let mut op = OperationConfig::new_default(OperationType::Adaptive3d);
    tweak(&mut op);
    let (mut session, index) = case_session(CaseSpec {
        name: "w6_5_produced",
        op,
        prior: if rest { Prior::RestPrior } else { Prior::None },
        stock_source: if rest {
            StockSource::FromRemainingStock
        } else {
            StockSource::Fresh
        },
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    });
    if rest {
        run_prior(&mut session);
    }
    let cancel = AtomicBool::new(false);
    match session.generate_toolpath(index, &cancel) {
        Ok(result) => VolumetricProduced::of(&result.annotated().produced),
        Err(e) => VolumetricProduced::refused(&e.to_string()),
    }
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/volumetric_produced_w6_5.toml")
}

const HEADER: &str = "\
# W6.5: blessed digests of the 3D Rough side channel (`Produced`).
# Written by tests/the_volumetric_wrapper_keeps_its_produced_w6_5.rs with
# RS_CAM_BLESS_PRODUCED_W6_5=1. Do not edit by hand.

";

#[test]
fn the_volumetric_wrapper_keeps_its_produced() {
    let actual: BTreeMap<String, VolumetricProduced> = ARMS
        .par_iter()
        .map(|&(arm, tweak, rest)| (format!("adaptive3d/{arm}"), run(tweak, rest)))
        .collect();

    // Non-vacuity: the By Area ContourSpiral arm carries both items, and
    // the Global ContourParallel arm carries neither.
    let spiral = &actual["adaptive3d/by_area_helix_spiral"];
    assert!(
        spiral.engagement_samples > 0 && spiral.area_region_count > 0,
        "the By Area ContourSpiral arm must carry engagement samples and areas: {spiral:?}"
    );
    let fresh = &actual["adaptive3d/fresh"];
    assert!(
        fresh.engagement_samples == 0 && fresh.area_region_count == 0,
        "the default arm carries a side channel: {fresh:?}"
    );

    if std::env::var_os(BLESS_ENV).is_some() {
        let body = toml::to_string(&actual).expect("the digests serialize");
        std::fs::write(golden_path(), format!("{HEADER}{body}")).expect("write the digests");
        eprintln!(
            "blessed {} arms into {}",
            actual.len(),
            golden_path().display()
        );
        return;
    }
    let text = std::fs::read_to_string(golden_path())
        .unwrap_or_else(|e| panic!("missing blessed file ({e}); bless it with {BLESS_ENV}=1"));
    let expected: BTreeMap<String, VolumetricProduced> =
        toml::from_str(&text).expect("the blessed file parses");
    let changed: Vec<String> = expected
        .keys()
        .chain(actual.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|k| expected.get(*k) != actual.get(*k))
        .map(|k| format!("{k}: {:?} -> {:?}", expected.get(k), actual.get(k)))
        .collect();
    assert!(
        changed.is_empty(),
        "the 3D Rough side channel moved. Re-bless with {BLESS_ENV}=1 only for an intended \
         change:\n{}",
        changed.join("\n")
    );
}
